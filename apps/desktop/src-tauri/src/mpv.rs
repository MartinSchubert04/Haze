//! mpv como motor de audio: un proceso aparte, sin ventana, que se maneja por su IPC
//! JSON en un named pipe. Reproduce cualquier formato y, con yt-dlp, URLs de YouTube.
//!
//! Es el [`Engine`] de escritorio: traduce los comandos del reproductor a mpv y lo que
//! avisa mpv a [`EngineEvent`].

use std::path::Path;
use std::time::Duration;

use haze_core::player::{Engine, EngineEvent, SharedPlayer};
use haze_core::song::Song;
use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::sync::mpsc;

use crate::tools;

pub struct Mpv {
    tx: mpsc::UnboundedSender<String>,
    child: std::process::Child,
}

/// Propiedades que mpv avisa solo cuando cambian.
const OBSERVED: &[&str] =
    &["pause", "time-pos", "duration", "volume", "idle-active", "media-title", "paused-for-cache", "current-ao"];

impl Mpv {
    pub async fn spawn(mpv: &Path, volume: f64, events: mpsc::UnboundedSender<EngineEvent>) -> Result<Self, String> {
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
                let Some(ev) = translate(&v) else { continue };
                if events.send(ev).is_err() {
                    return;
                }
            }
            let _ = events.send(EngineEvent::Exited);
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

    pub fn set(&self, prop: &str, value: Value) {
        self.command(json!(["set_property", prop, value]));
    }
}

impl Engine for Mpv {
    fn load(&self, song: &Song, start: f64, paused: bool) {
        let path = &song.path;
        self.set("pause", json!(paused));
        if start > 0.0 {
            self.command(json!(["loadfile", path, "replace", -1, format!("start={start}")]));
        } else {
            self.command(json!(["loadfile", path, "replace"]));
        }
    }

    fn set_paused(&self, paused: bool) {
        self.set("pause", json!(paused));
    }

    fn seek(&self, seconds: f64) {
        self.command(json!(["seek", seconds, "absolute"]));
    }

    fn set_volume(&self, volume: f64) {
        self.set("volume", json!(volume));
    }

    fn stop(&self) {
        self.command(json!(["stop"]));
    }
}

/// Pasa un mensaje del IPC de mpv a un evento del reproductor. Lo que no le importa al
/// reproductor (otras propiedades, respuestas a comandos) queda en `None`.
fn translate(v: &Value) -> Option<EngineEvent> {
    match v["event"].as_str()? {
        "property-change" => {
            let data = &v["data"];
            match v["name"].as_str()? {
                "time-pos" => data.as_f64().map(EngineEvent::Position),
                "duration" => data.as_f64().map(EngineEvent::Duration),
                "pause" => data.as_bool().map(EngineEvent::Paused),
                "paused-for-cache" => Some(EngineEvent::Buffering(data.as_bool().unwrap_or(false))),
                "current-ao" => Some(EngineEvent::AudioOutput(data.as_str().is_some_and(|ao| ao != "null"))),
                "media-title" => data.as_str().map(|t| EngineEvent::Title(t.to_string())),
                _ => None,
            }
        }
        // `reason`: eof, error, stop, quit, redirect. Solo importan los dos primeros: el
        // resto es porque se cargo otra cosa o se freno a proposito.
        "end-file" => match v["reason"].as_str()? {
            "eof" => Some(EngineEvent::Ended),
            "error" => Some(EngineEvent::Failed(v["file_error"].as_str().unwrap_or_default().to_string())),
            _ => None,
        },
        "file-loaded" => Some(EngineEvent::Loaded),
        _ => None,
    }
}

/// Procesa los eventos de mpv en orden, del lado del reproductor.
pub fn pump(player: SharedPlayer, mut rx: mpsc::UnboundedReceiver<EngineEvent>) {
    tauri::async_runtime::spawn(async move {
        let mut tick = tokio::time::interval(Duration::from_secs(1));
        loop {
            tokio::select! {
                ev = rx.recv() => match ev {
                    Some(ev) => player.lock().await.on_event(ev),
                    None => return,
                },
                _ = tick.tick() => player.lock().await.check_audio(),
            }
        }
    });
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

#[cfg(test)]
mod tests {
    use super::*;

    fn prop(name: &str, data: Value) -> Option<EngineEvent> {
        translate(&json!({ "event": "property-change", "name": name, "data": data }))
    }

    #[test]
    fn translates_properties() {
        assert_eq!(prop("time-pos", json!(1.5)), Some(EngineEvent::Position(1.5)));
        assert_eq!(prop("time-pos", Value::Null), None);
        assert_eq!(prop("pause", json!(true)), Some(EngineEvent::Paused(true)));
        assert_eq!(prop("paused-for-cache", Value::Null), Some(EngineEvent::Buffering(false)));
        assert_eq!(prop("current-ao", json!("wasapi")), Some(EngineEvent::AudioOutput(true)));
        assert_eq!(prop("current-ao", json!("null")), Some(EngineEvent::AudioOutput(false)));
        assert_eq!(prop("current-ao", Value::Null), Some(EngineEvent::AudioOutput(false)));
        assert_eq!(prop("volume", json!(50)), None);
    }

    #[test]
    fn translates_end_file() {
        let end = |reason: &str| translate(&json!({ "event": "end-file", "reason": reason, "file_error": "boom" }));
        assert_eq!(end("eof"), Some(EngineEvent::Ended));
        assert_eq!(end("error"), Some(EngineEvent::Failed("boom".into())));
        assert_eq!(end("stop"), None);
        assert_eq!(translate(&json!({ "event": "file-loaded" })), Some(EngineEvent::Loaded));
        assert_eq!(translate(&json!({ "request_id": 0, "error": "success" })), None);
    }
}
