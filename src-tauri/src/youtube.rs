//! YouTube (y lo demas que soporte yt-dlp: SoundCloud, Bandcamp...): buscar, escuchar
//! sin bajar y bajar como MP3 con titulo, artista y tapa.

use std::path::Path;
use std::sync::Arc;

use serde::Serialize;
use serde_json::Value;
use tauri::{AppHandle, Emitter, Manager};
use tokio::io::{AsyncBufReadExt, BufReader};

use crate::library::{self, Song, SongKind};
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

fn thumb_for(id: &str) -> String {
    format!("https://i.ytimg.com/vi/{id}/mqdefault.jpg")
}

fn stream_song(v: &Value) -> Option<Song> {
    let id = v["id"].as_str()?;
    let url = v["webpage_url"]
        .as_str()
        .or_else(|| v["url"].as_str().filter(|u| u.starts_with("http")))
        .map(str::to_string)
        .unwrap_or_else(|| format!("https://www.youtube.com/watch?v={id}"));
    let is_youtube = url.contains("youtube.com") || url.contains("youtu.be");
    let thumb = if is_youtube {
        Some(thumb_for(id))
    } else {
        v["thumbnail"].as_str().map(str::to_string).or_else(|| {
            v["thumbnails"].as_array().and_then(|t| t.last()).and_then(|t| t["url"].as_str()).map(str::to_string)
        })
    };
    let artist = v["artist"]
        .as_str()
        .or_else(|| v["channel"].as_str())
        .or_else(|| v["uploader"].as_str())
        .unwrap_or_default()
        .trim_end_matches(" - Topic")
        .to_string();
    Some(Song {
        id: library::id_for(&url),
        path: url,
        title: v["title"].as_str().unwrap_or_default().to_string(),
        artist,
        album: String::new(),
        duration_ms: (v["duration"].as_f64().unwrap_or(0.0) * 1000.0) as u64,
        mtime: 0,
        folder: "YouTube".into(),
        kind: SongKind::Stream,
        thumb,
        has_cover: false,
    })
}

async fn ytdlp_json(app: &AppHandle, args: &[&str]) -> Result<Value, String> {
    let ytdlp = ensure_tool(app, Tool::YtDlp).await?;
    let out = tools::async_command(&ytdlp)
        .args(["--ignore-config", "--no-warnings", "-J", "--flat-playlist"])
        .args(args)
        .output()
        .await
        .map_err(|e| e.to_string())?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr);
        let line = err.lines().rev().find(|l| l.contains("ERROR")).unwrap_or("yt-dlp fallo");
        return Err(line.replace("ERROR: ", ""));
    }
    serde_json::from_slice(&out.stdout).map_err(|e| e.to_string())
}

pub async fn search(app: &AppHandle, query: &str) -> Result<Vec<Song>, String> {
    let q = format!("ytsearch25:{query}");
    let v = ytdlp_json(app, &[&q]).await?;
    Ok(v["entries"].as_array().into_iter().flatten().filter_map(stream_song).collect())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LinkInfo {
    pub title: String,
    pub is_playlist: bool,
    pub count: usize,
    pub thumb: Option<String>,
    pub songs: Vec<Song>,
}

/// Que hay detras de un link pegado: un video o una playlist.
pub async fn inspect(app: &AppHandle, url: &str) -> Result<LinkInfo, String> {
    let v = ytdlp_json(app, &[url]).await?;
    if v["_type"].as_str() == Some("playlist") {
        let songs: Vec<Song> = v["entries"].as_array().into_iter().flatten().filter_map(stream_song).collect();
        Ok(LinkInfo {
            title: v["title"].as_str().unwrap_or("Playlist").to_string(),
            is_playlist: true,
            count: songs.len(),
            thumb: songs.first().and_then(|s| s.thumb.clone()),
            songs,
        })
    } else {
        let song = stream_song(&v).ok_or("No encontre nada para reproducir en ese link")?;
        Ok(LinkInfo { title: song.title.clone(), is_playlist: false, count: 1, thumb: song.thumb.clone(), songs: vec![song] })
    }
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

    let output = if job.playlist { "%(playlist_title)s/%(title)s.%(ext)s" } else { "%(title)s.%(ext)s" };
    // Tapas cuadradas, como las de un disco: se recorta el centro de la miniatura.
    let crop = r#"ThumbnailsConvertor+FFmpeg_o:-c:v mjpeg -qmin 1 -qscale:v 2 -vf crop="'if(gt(ih,iw),iw,ih)':'if(gt(iw,ih),ih,iw)'""#;
    let mut child = tools::async_command(&ytdlp)
        .args([
            "--ignore-config",
            "--no-warnings",
            "--newline",
            "--windows-filenames",
            "-x",
            "--audio-format",
            "mp3",
            "--audio-quality",
            "0",
            "--embed-metadata",
            "--embed-thumbnail",
            "--convert-thumbnails",
            "jpg",
            "--ppa",
            crop,
            "--replace-in-metadata",
            "uploader,channel,artist",
            " - Topic$",
            "",
            "--progress-template",
            "download:DL %(progress._percent_str)s",
            "--print",
            "after_move:FILE %(filepath)s",
            "-P",
            &job.folder,
            "-o",
            output,
        ])
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
        let line = line.trim();
        if let Some(rest) = line.strip_prefix("DL ") {
            if let Ok(p) = rest.trim().trim_end_matches('%').trim().parse::<f32>() {
                update_job(app, job.id, |j| {
                    j.status = "downloading".into();
                    j.progress = p / 100.0;
                });
            }
        } else if line.contains("[ExtractAudio]") || line.contains("[EmbedThumbnail]") {
            update_job(app, job.id, |j| j.status = "converting".into());
        } else if let Some(rest) = line.strip_prefix("[download] Downloading item ") {
            let item = rest.replace(" of ", " de ");
            update_job(app, job.id, |j| j.item = Some(item));
        } else if let Some(path) = line.strip_prefix("FILE ") {
            let path = path.to_string();
            add_to_library(app, Path::new(&path));
            update_job(app, job.id, |j| j.files.push(path.replace('\\', "/")));
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
