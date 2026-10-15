//! mpv como motor de audio: un proceso aparte, sin ventana, que se maneja por su IPC
//! JSON en un named pipe. Reproduce cualquier formato y, con yt-dlp, URLs de YouTube.

use std::path::Path;
use std::time::Duration;

use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::sync::mpsc;

use crate::tools;

#[derive(Debug)]
pub enum MpvEvent {
    Property(String, Value),
    /// Termino un archivo. `reason`: eof, error, stop, quit, redirect.
    EndFile { reason: String, error: Option<String> },
    FileLoaded,
    /// El proceso se cerro (o se rompio el pipe).
    Exited,
}

pub struct Mpv {
    tx: mpsc::UnboundedSender<String>,
    child: std::process::Child,
}

/// Propiedades que mpv avisa solo cuando cambian.
const OBSERVED: &[&str] =
    &["pause", "time-pos", "duration", "volume", "idle-active", "media-title", "paused-for-cache", "current-ao"];

impl Mpv {
    pub async fn spawn(mpv: &Path, volume: f64, events: mpsc::UnboundedSender<MpvEvent>) -> Result<Self, String> {
        let pipe = format!(r"\\.\pipe\haze-mpv-{}", std::process::id());
        let mut cmd = tools::command(mpv);
        cmd.args([
            "--idle=yes",
            "--no-config",
            "--vid=no",
            "--video=no",
            "--audio-display=no",
            "--force-window=no",
            "--terminal=no",
            "--keep-open=no",
            "--volume-max=100",
            "--gapless-audio=weak",
            // Sin dispositivo de audio, mpv reintenta en loop y se come un nucleo.
            // Con esto pasa a una salida nula y la app avisa (ver check_audio).
            "--audio-fallback-to-null=yes",
            "--audio-client-name=Haze",
            "--ytdl-format=bestaudio/best",
            "--cache=yes",
            "--demuxer-max-bytes=32MiB",
        ]);
        cmd.arg(format!("--volume={volume}"));
        // Para probar sin parlantes: HAZE_MPV_ARGS="--ao=null".
        if let Ok(extra) = std::env::var("HAZE_MPV_ARGS") {
            cmd.args(extra.split_whitespace());
        }
        cmd.arg(format!("--input-ipc-server={pipe}"));
        if let Some(ytdlp) = tools::find(tools::Tool::YtDlp) {
            cmd.arg(format!("--script-opts=ytdl_hook-ytdl_path={}", ytdlp.to_string_lossy()));
        }
        let child = cmd.spawn().map_err(|e| format!("No se pudo abrir mpv: {e}"))?;
        kill_with_parent(&child);

        // mpv tarda unos milisegundos en crear el pipe.
        let mut client = None;
        for _ in 0..100 {
            match tokio::net::windows::named_pipe::ClientOptions::new().open(&pipe) {
                Ok(c) => {
                    client = Some(c);
                    break;
                }
                Err(_) => tokio::time::sleep(Duration::from_millis(50)).await,
            }
        }
        let client = client.ok_or("mpv no abrio su canal de control")?;
        let (read, mut write) = tokio::io::split(client);

        let (tx, mut rx) = mpsc::unbounded_channel::<String>();
        tauri::async_runtime::spawn(async move {
            while let Some(line) = rx.recv().await {
                if write.write_all(line.as_bytes()).await.is_err() || write.write_all(b"\n").await.is_err() {
                    break;
                }
            }
        });

        tauri::async_runtime::spawn(async move {
            let mut lines = BufReader::new(read).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                let Ok(v) = serde_json::from_str::<Value>(&line) else { continue };
                let ev = match v["event"].as_str() {
                    Some("property-change") => {
                        MpvEvent::Property(v["name"].as_str().unwrap_or_default().to_string(), v["data"].clone())
                    }
                    Some("end-file") => MpvEvent::EndFile {
                        reason: v["reason"].as_str().unwrap_or_default().to_string(),
                        error: v["file_error"].as_str().map(str::to_string),
                    },
                    Some("file-loaded") => MpvEvent::FileLoaded,
                    _ => continue,
                };
                if events.send(ev).is_err() {
                    return;
                }
            }
            let _ = events.send(MpvEvent::Exited);
        });

        let me = Self { tx, child };
        for (i, name) in OBSERVED.iter().enumerate() {
            me.command(json!(["observe_property", i + 1, name]));
        }
        Ok(me)
    }

    pub fn command(&self, args: Value) {
        let _ = self.tx.send(json!({ "command": args }).to_string());
    }

    pub fn load(&self, path: &str, start: f64) {
        if start > 0.0 {
            self.command(json!(["loadfile", path, "replace", -1, format!("start={start}")]));
        } else {
            self.command(json!(["loadfile", path, "replace"]));
        }
    }

    pub fn set(&self, prop: &str, value: Value) {
        self.command(json!(["set_property", prop, value]));
    }

    pub fn stop(&self) {
        self.command(json!(["stop"]));
    }
}

impl Drop for Mpv {
    fn drop(&mut self) {
        let _ = self.tx.send(json!({ "command": ["quit"] }).to_string());
        std::thread::sleep(Duration::from_millis(100));
        let _ = self.child.kill();
    }
}

/// Mete a mpv en un Job de Windows que muere con este proceso: si la app se cuelga o la
/// matan desde el administrador de tareas, mpv no queda sonando huerfano.
#[cfg(windows)]
fn kill_with_parent(child: &std::process::Child) {
    use std::os::windows::io::AsRawHandle;
    use windows::Win32::Foundation::HANDLE;
    use windows::Win32::System::JobObjects::*;

    static JOB: std::sync::OnceLock<usize> = std::sync::OnceLock::new();
    unsafe {
        let job = *JOB.get_or_init(|| {
            let Ok(job) = CreateJobObjectW(None, None) else { return 0 };
            let mut info = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
            info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
            let _ = SetInformationJobObject(
                job,
                JobObjectExtendedLimitInformation,
                &info as *const _ as *const _,
                std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
            );
            job.0 as usize
        });
        if job != 0 {
            let _ = AssignProcessToJobObject(HANDLE(job as _), HANDLE(child.as_raw_handle()));
        }
    }
}

#[cfg(not(windows))]
fn kill_with_parent(_child: &std::process::Child) {}
