//! YouTube (y lo demas que soporte yt-dlp: SoundCloud, Bandcamp...): buscar, escuchar
//! sin bajar y bajar como MP3 con titulo, artista y tapa.

use std::path::Path;
use std::sync::Arc;

use haze_core::youtube::{self as yt, Progress};
pub use haze_core::youtube::LinkInfo;
use serde::Serialize;
use serde_json::Value;
use tauri::{AppHandle, Emitter, Manager};
use tokio::io::{AsyncBufReadExt, BufReader};

use crate::library::{self, Song};
use crate::tools::{self, Tool};
use crate::AppState;

/// Asegura un programa avisando el progreso de la descarga a la interfaz.
pub async fn ensure_tool(app: &AppHandle, tool: Tool) -> Result<std::path::PathBuf, String> {
    if let Some(p) = tools::find(tool) {
        return Ok(p);
    }
    let app2 = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        tools::ensure(tool, &|p| {
            let _ = app2.emit("tools://progress", serde_json::json!({ "tool": tool.label(), "progress": p }));
        })
    })
    .await
    .map_err(|e| e.to_string())?
}

async fn ytdlp_json(app: &AppHandle, args: &[&str]) -> Result<Value, String> {
    let ytdlp = ensure_tool(app, Tool::YtDlp).await?;
    let out = tools::async_command(&ytdlp)
        .args(yt::JSON_ARGS)
        .args(args)
        .output()
        .await
        .map_err(|e| e.to_string())?;
    if !out.status.success() {
        return Err(yt::error_line(&String::from_utf8_lossy(&out.stderr)).unwrap_or_else(|| "yt-dlp fallo".into()));
    }
    serde_json::from_slice(&out.stdout).map_err(|e| e.to_string())
}

pub async fn search(app: &AppHandle, query: &str) -> Result<Vec<Song>, String> {
    Ok(yt::search_results(&ytdlp_json(app, &[&yt::search_query(query)]).await?))
}

/// Que hay detras de un link pegado: un video o una playlist.
pub async fn inspect(app: &AppHandle, url: &str) -> Result<LinkInfo, String> {
    yt::link_info(&ytdlp_json(app, &[url]).await?)
}

// ------------------------------------------------------------------ descargas

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadJob {
    pub id: u64,
    pub url: String,
    pub title: String,
    pub thumb: Option<String>,
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
    let state = app.state::<AppState>();
    let jobs = state.downloads.lock().unwrap().jobs.clone();
    let _ = app.emit("downloads://changed", jobs);
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

pub fn enqueue(app: &AppHandle, url: String, title: String, thumb: Option<String>, folder: String, playlist: bool) -> u64 {
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
                folder,
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
        (id, start)
    };
    emit_jobs(app);
    if start {
        let app = app.clone();
        tauri::async_runtime::spawn(async move { worker(app).await });
    }
    id
}

pub fn clear_finished(app: &AppHandle) {
    {
        let state = app.state::<AppState>();
        state.downloads.lock().unwrap().jobs.retain(|j| j.status == "queued" || j.status == "downloading" || j.status == "converting");
    }
    emit_jobs(app);
}

/// Baja de a una: varias en paralelo solo harian que YouTube limite la velocidad.
async fn worker(app: AppHandle) {
    loop {
        let next = {
            let state = app.state::<AppState>();
            let mut d = state.downloads.lock().unwrap();
            let next = d.jobs.iter().rev().find(|j| j.status == "queued").cloned();
            if next.is_none() {
                d.running = false;
            }
            next
        };
        let Some(job) = next else { return };
        if let Err(e) = run_job(&app, &job).await {
            update_job(&app, job.id, |j| {
                j.status = "error".into();
                j.error = Some(e);
            });
        }
    }
}

async fn run_job(app: &AppHandle, job: &DownloadJob) -> Result<(), String> {
    update_job(app, job.id, |j| j.status = "downloading".into());
    let ytdlp = ensure_tool(app, Tool::YtDlp).await?;
    ensure_tool(app, Tool::Ffmpeg).await?;
    // yt-dlp necesita un motor de JavaScript para sacar los formatos de YouTube.
    ensure_tool(app, Tool::Deno).await?;

    let mut child = tools::async_command(&ytdlp)
        .args(yt::DOWNLOAD_ARGS)
        .args(["--windows-filenames", "-P", &job.folder, "-o", yt::output_template(job.playlist)])
        .arg(if job.playlist { "--yes-playlist" } else { "--no-playlist" })
        .arg(&job.url)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| e.to_string())?;

    let stdout = child.stdout.take().unwrap();
    let stderr = child.stderr.take().unwrap();
    let err_task = tauri::async_runtime::spawn(async move {
        let mut lines = BufReader::new(stderr).lines();
        let mut last = String::new();
        while let Ok(Some(l)) = lines.next_line().await {
            if l.contains("ERROR") {
                last = l.replace("ERROR: ", "");
            }
        }
        last
    });

    let mut lines = BufReader::new(stdout).lines();
    while let Ok(Some(line)) = lines.next_line().await {
        match yt::parse_progress(&line) {
            Some(Progress::Downloading(p)) => update_job(app, job.id, |j| {
                j.status = "downloading".into();
                j.progress = p;
            }),
            Some(Progress::Converting) => update_job(app, job.id, |j| j.status = "converting".into()),
            Some(Progress::Item(item)) => update_job(app, job.id, |j| j.item = Some(item)),
            Some(Progress::File(path)) => {
                add_to_library(app, Path::new(&path));
                update_job(app, job.id, |j| j.files.push(path.replace('\\', "/")));
            }
            None => {}
        }
    }

    let status = child.wait().await.map_err(|e| e.to_string())?;
    let err = err_task.await.unwrap_or_default();
    let got_files = {
        let state = app.state::<AppState>();
        let d = state.downloads.lock().unwrap();
        d.jobs.iter().find(|j| j.id == job.id).map(|j| !j.files.is_empty()).unwrap_or(false)
    };
    if status.success() || got_files {
        update_job(app, job.id, |j| {
            j.status = "done".into();
            j.progress = 1.0;
            // En una playlist algun video puede fallar (privado, bloqueado) sin que se
            // pierda el resto: se avisa pero cuenta como hecha.
            if !status.success() && !err.is_empty() {
                j.error = Some(err.clone());
            }
        });
        let _ = app.emit("downloads://done", job.title.clone());
        Ok(())
    } else {
        Err(if err.is_empty() { "yt-dlp termino con error".into() } else { err })
    }
}

/// Suma a la biblioteca lo recien bajado, si cae dentro de alguna carpeta fuente.
pub fn add_to_library(app: &AppHandle, path: &Path) {
    let state = app.state::<AppState>();
    let sources = state.settings.lock().unwrap().sources.clone();
    let norm = library::normalize(path).to_lowercase();
    let inside = sources.iter().any(|s| norm.starts_with(&s.to_lowercase().trim_end_matches('/').to_string()));
    if !inside {
        return;
    }
    if let Some(song) = library::read_file(path) {
        let song: Arc<Song> = state.library.write().unwrap().upsert(song);
        let _ = app.emit("library://added", &*song);
        let lib = state.library.read().unwrap();
        lib.save_cache(&state.cache_path);
    }
}
