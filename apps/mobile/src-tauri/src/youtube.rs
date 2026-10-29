//! YouTube en el telefono: buscar, escuchar sin bajar y bajar como MP3 a Music/Haze.
//! yt-dlp corre en Kotlin (`youtubedl-android`); lo que se le pasa y como se lee lo que
//! devuelve es lo mismo que en escritorio (`haze_core::youtube`).

use std::sync::atomic::{AtomicBool, Ordering};

use haze_core::song::Song;
use haze_core::youtube::{self as yt, LinkInfo, Progress};
use serde::Serialize;
use serde_json::Value;
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_haze_player::{DownloadsStatus, HazePlayerExt};

use crate::AppState;

async fn ytdlp_json(app: &AppHandle, args: Vec<String>) -> Result<Value, String> {
    let h = app.clone();
    let out = tauri::async_runtime::spawn_blocking(move || {
        let all: Vec<&str> = yt::JSON_ARGS.iter().copied().chain(args.iter().map(String::as_str)).collect();
        h.haze_player().yt_run(&all)
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| clean_error(&e.to_string()))?;
    serde_json::from_str(&out).map_err(|e| e.to_string())
}

/// El error de yt-dlp llega con todo su stderr: se queda la linea que importa.
fn clean_error(e: &str) -> String {
    yt::error_line(e).unwrap_or_else(|| e.lines().last().unwrap_or("yt-dlp fallo").to_string())
}

#[tauri::command]
pub async fn yt_search(app: AppHandle, query: String) -> Result<Vec<Song>, String> {
    Ok(yt::search_results(&ytdlp_json(&app, vec![yt::search_query(&query)]).await?))
}

#[tauri::command]
pub async fn yt_inspect(app: AppHandle, url: String) -> Result<LinkInfo, String> {
    yt::link_info(&ytdlp_json(&app, vec![url]).await?)
}

// ------------------------------------------------------------------ descargas

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadJob {
    pub id: u64,
    pub url: String,
    pub title: String,
    pub thumb: Option<String>,
    /// Donde termina: "Music/Haze".
    pub folder: String,
    pub playlist: bool,
    /// queued, downloading, converting, done, error
    pub status: String,
    pub progress: f32,
    /// "3 de 12" en playlists.
    pub item: Option<String>,
    pub error: Option<String>,
    pub files: Vec<String>,
}

#[derive(Default)]
pub struct Downloads {
    pub jobs: Vec<DownloadJob>,
    next_id: u64,
    running: bool,
}

fn emit_jobs(app: &AppHandle) {
    let jobs = app.state::<AppState>().downloads.lock().unwrap().jobs.clone();
    notify(app, &jobs);
    let _ = app.emit("downloads://changed", jobs);
}

/// Pasa lo que se esta bajando a la notificacion del servicio que sostiene las descargas
/// con la app en el fondo. Android la actualiza cada tanto, no en cada linea.
fn notify(app: &AppHandle, jobs: &[DownloadJob]) {
    let Some(current) = jobs.iter().find(|j| j.status == "downloading" || j.status == "converting") else { return };
    let queued = jobs.iter().filter(|j| j.status == "queued").count();
    let mut detail: Vec<String> = Vec::new();
    if let Some(item) = &current.item {
        detail.push(item.clone());
    }
    if current.status == "downloading" {
        detail.push(format!("{:.0}%", current.progress * 100.0));
    }
    if queued > 0 {
        detail.push(format!("+{queued} en cola"));
    }
    app.haze_player().downloads_status(DownloadsStatus::Progress {
        title: current.title.clone(),
        progress: current.progress,
        converting: current.status == "converting",
        detail: detail.join(" · "),
    });
}

fn update_job(app: &AppHandle, id: u64, f: impl FnOnce(&mut DownloadJob)) {
    {
        let state = app.state::<AppState>();
        let mut d = state.downloads.lock().unwrap();
        if let Some(j) = d.jobs.iter_mut().find(|j| j.id == id) {
            f(j);
        }
    }
    emit_jobs(app);
}

/// Una linea de yt-dlp de la descarga `id` (llega por el canal del plugin).
pub fn on_line(app: &AppHandle, id: u64, line: &str) {
    match yt::parse_progress(line) {
        Some(Progress::Downloading(p)) => update_job(app, id, |j| {
            j.status = "downloading".into();
            j.progress = p;
        }),
        Some(Progress::Converting) => update_job(app, id, |j| j.status = "converting".into()),
        Some(Progress::Item(item)) => update_job(app, id, |j| j.item = Some(item)),
        // La ruta es la temporal: lo que importa es el nombre, que queda igual.
        Some(Progress::File(path)) => {
            let name = path.rsplit(['/', '\\']).next().unwrap_or(&path).to_string();
            update_job(app, id, |j| j.files.push(name));
        }
        None => {}
    }
}

#[tauri::command]
pub fn yt_download(app: AppHandle, url: String, title: String, thumb: Option<String>, playlist: bool) -> u64 {
    let state = app.state::<AppState>();
    let (id, start) = {
        let mut d = state.downloads.lock().unwrap();
        d.next_id += 1;
        let id = d.next_id;
        d.jobs.insert(
            0,
            DownloadJob {
                id,
                url,
                title,
                thumb,
                folder: "Music/Haze".into(),
                playlist,
                status: "queued".into(),
                progress: 0.0,
                item: None,
                error: None,
                files: vec![],
            },
        );
        let start = !d.running;
        d.running = true;
        // Con la app a la vista, que es cuando Android deja arrancar el servicio que la
        // sostiene en el fondo. Bajo el lock: asi no se cruza con el aviso de que termino.
        app.haze_player().downloads_status(DownloadsStatus::Start);
        (id, start)
    };
    emit_jobs(&app);
    ask_notifications(&app);
    if start {
        tauri::async_runtime::spawn(worker(app.clone()));
    }
    id
}

/// El avance de las descargas en el fondo se ve en una notificacion: Android 13+ pide
/// permiso. Se pregunta con la primera descarga, una vez por sesion.
fn ask_notifications(app: &AppHandle) {
    static ASKED: AtomicBool = AtomicBool::new(false);
    if ASKED.swap(true, Ordering::Relaxed) {
        return;
    }
    let h = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        if let Err(e) = h.haze_player().request_notifications() {
            log::warn!("haze: no se pudo pedir el permiso de notificaciones: {e}");
        }
    });
}

#[tauri::command]
pub fn downloads_clear(app: AppHandle) {
    app.state::<AppState>()
        .downloads
        .lock()
        .unwrap()
        .jobs
        .retain(pending);
    emit_jobs(&app);
}

/// El ffmpeg de `youtubedl-android` no carga en todos los telefonos (los de paginas de
/// 16 KB). Despues del primer fallo se baja sin convertir.
static NO_FFMPEG: AtomicBool = AtomicBool::new(false);

fn pending(j: &DownloadJob) -> bool {
    j.status == "queued" || j.status == "downloading" || j.status == "converting"
}

/// Baja de a una: varias en paralelo solo harian que YouTube limite la velocidad.
async fn worker(app: AppHandle) {
    let (mut done, mut failed, mut last) = (0, 0, String::new());
    loop {
        let next = {
            let state = app.state::<AppState>();
            let mut d = state.downloads.lock().unwrap();
            let next = d.jobs.iter().rev().find(|j| j.status == "queued").cloned();
            if next.is_none() {
                d.running = false;
                // Se va el servicio y, con la app en el fondo, queda un aviso de como termino.
                app.haze_player().downloads_status(DownloadsStatus::Idle { done, failed, last: last.clone() });
            }
            next
        };
        let Some(job) = next else { return };
        last = job.title.clone();
        update_job(&app, job.id, |j| j.status = "downloading".into());
        let mut result = download(&app, &job, !NO_FFMPEG.load(Ordering::Relaxed)).await;
        if let Err(e) = &result {
            if yt::missing_ffmpeg(e) && !NO_FFMPEG.swap(true, Ordering::Relaxed) {
                log::warn!("haze: ffmpeg no anda en este telefono, se baja sin convertir");
                result = download(&app, &job, false).await;
            }
        }
        let id = job.id;
        match result {
            // yt-dlp termino bien pero no quedo ningun audio: es una falla.
            Ok(0) => {
                failed += 1;
                update_job(&app, id, |j| {
                    j.status = "error".into();
                    j.error = Some("No se guardó ningún archivo".into());
                });
            }
            Ok(_) => {
                done += 1;
                update_job(&app, id, |j| {
                    j.status = "done".into();
                    j.progress = 1.0;
                });
                let _ = app.emit("downloads://done", job.title.clone());
                // MediaStore ya los tiene: se vuelve a leer la biblioteca.
                crate::load_library(&app).await;
            }
            Err(e) => {
                failed += 1;
                update_job(&app, id, |j| {
                    j.status = "error".into();
                    j.error = Some(e);
                });
            }
        }
    }
}

/// Corre yt-dlp para una descarga. Devuelve cuantos archivos guardo.
async fn download(app: &AppHandle, job: &DownloadJob, ffmpeg: bool) -> Result<usize, String> {
    let mut args: Vec<String> = if ffmpeg {
        yt::DOWNLOAD_ARGS.iter().map(|s| s.to_string()).collect()
    } else {
        yt::download_args_without_ffmpeg().into_iter().map(String::from).collect()
    };
    args.extend([
        // Nombres que acepta cualquier sistema de archivos del telefono.
        "--windows-filenames".to_string(),
        "-o".into(),
        yt::output_template(job.playlist).into(),
        if job.playlist { "--yes-playlist" } else { "--no-playlist" }.into(),
        job.url.clone(),
    ]);
    let h = app.clone();
    let id = job.id;
    tauri::async_runtime::spawn_blocking(move || {
        let all: Vec<&str> = args.iter().map(String::as_str).collect();
        h.haze_player().yt_download(id, &all)
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| clean_error(&e))
}
