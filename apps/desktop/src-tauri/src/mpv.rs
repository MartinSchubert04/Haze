//! mpv como motor de audio: un proceso aparte, sin ventana, que se maneja por su IPC
//! JSON en un named pipe. Reproduce cualquier formato y, con yt-dlp, URLs de YouTube.
//!
//! Es el [`Engine`] de escritorio: traduce los comandos del reproductor a mpv y lo que
//! avisa mpv a [`EngineEvent`].

use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use haze_core::player::{Engine, EngineEvent, SharedPlayer};
use haze_core::song::Song;
use serde::Serialize;
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
    pub async fn spawn(
        mpv: &Path,
        volume: f64,
        output: Output,
        events: mpsc::UnboundedSender<EngineEvent>,
    ) -> Result<Self, String> {
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

        output.attach(tx.clone());
        tauri::async_runtime::spawn(async move {
            let mut lines = BufReader::new(read).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                let Ok(v) = serde_json::from_str::<Value>(&line) else { continue };
                if let Some(list) = device_list(&v) {
                    output.set_devices(list);
                    continue;
                }
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
        // mpv la manda apenas se observa y de nuevo cada vez que se enchufa o desenchufa algo.
        me.command(json!(["observe_property", 0, "audio-device-list"]));
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

    fn reopen_audio(&self) {
        self.command(json!(["ao-reload"]));
    }

    fn stop(&self) {
        self.command(json!(["stop"]));
    }
}

/// Una salida de audio de Windows, como la nombra mpv.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct AudioDevice {
    /// "wasapi/{...}": lo que se guarda en Ajustes.
    pub id: String,
    /// "Altavoces (HyperX Cloud II Wireless)".
    pub name: String,
}

/// La salida de audio elegida en Ajustes. Vive en el estado de la app aunque mpv todavia no
/// haya arrancado (o se reinicie): cuando mpv avisa que dispositivos hay, se le pone el
/// elegido si esta conectado y, si no, el de Windows. Asi, si se apaga la tele, la musica
/// sigue por los parlantes en vez de quedar muda, y vuelve a la tele cuando se prende.
#[derive(Clone, Default)]
pub struct Output(Arc<Mutex<OutputState>>);

#[derive(Default)]
struct OutputState {
    tx: Option<mpsc::UnboundedSender<String>>,
    /// `None` hasta que mpv manda la lista por primera vez.
    devices: Option<Vec<AudioDevice>>,
    /// El id elegido; vacio es el de Windows.
    wanted: String,
    /// Lo que tiene puesto mpv ahora.
    applied: String,
}

impl Output {
    pub fn new(wanted: &str) -> Self {
        let out = Self::default();
        out.0.lock().unwrap().wanted = wanted.to_string();
        out
    }

    /// Las salidas conectadas. Vacio si mpv todavia no arranco.
    pub fn devices(&self) -> Vec<AudioDevice> {
        self.0.lock().unwrap().devices.clone().unwrap_or_default()
    }

    /// Cambia de salida. Si mpv ya esta sonando, se pasa sin cortar el tema.
    pub fn select(&self, id: &str) {
        let mut st = self.0.lock().unwrap();
        st.wanted = id.to_string();
        st.apply();
    }

    fn attach(&self, tx: mpsc::UnboundedSender<String>) {
        let mut st = self.0.lock().unwrap();
        st.tx = Some(tx);
        // mpv arranca en "auto"; la lista llega enseguida y ahi se aplica lo elegido.
        st.applied = "auto".into();
        st.devices = None;
    }

    fn set_devices(&self, devices: Vec<AudioDevice>) {
        let mut st = self.0.lock().unwrap();
        st.devices = Some(devices);
        st.apply();
    }
}

impl OutputState {
    /// El dispositivo que le corresponde a mpv: el elegido si esta conectado.
    fn target(&self) -> Option<&str> {
        let devices = self.devices.as_ref()?;
        Some(match devices.iter().find(|d| d.id == self.wanted) {
            Some(d) => &d.id,
            None => "auto",
        })
    }

    fn apply(&mut self) {
        let Some(target) = self.target().map(str::to_string) else { return };
        let Some(tx) = &self.tx else { return };
        if target != self.applied {
            let _ = tx.send(json!({ "command": ["set_property", "audio-device", target] }).to_string());
            self.applied = target;
        }
    }
}

/// La lista de salidas, si el mensaje es el aviso de `audio-device-list`. Quedan solo las de
/// WASAPI: "auto" es la opcion "la de Windows" de Ajustes y openal es otra forma de llegar a
/// las mismas.
fn device_list(v: &Value) -> Option<Vec<AudioDevice>> {
    if v["event"] != "property-change" || v["name"] != "audio-device-list" {
        return None;
    }
    let list = v["data"].as_array().map(Vec::as_slice).unwrap_or_default();
    Some(
        list.iter()
            .filter_map(|d| {
                let id = d["name"].as_str()?;
                id.starts_with("wasapi/").then(|| AudioDevice {
                    id: id.to_string(),
                    name: d["description"].as_str().unwrap_or(id).to_string(),
                })
            })
            .collect(),
    )
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
    fn reads_device_list() {
        let msg = json!({ "event": "property-change", "name": "audio-device-list", "data": [
            { "name": "auto", "description": "Autoselect device" },
            { "name": "wasapi/{a}", "description": "Auris" },
            { "name": "wasapi/{b}", "description": "SAMSUNG" },
            { "name": "openal", "description": "Default (openal)" },
        ]});
        let list = device_list(&msg).unwrap();
        assert_eq!(list.iter().map(|d| d.name.as_str()).collect::<Vec<_>>(), ["Auris", "SAMSUNG"]);
        assert_eq!(list[1].id, "wasapi/{b}");
        assert_eq!(device_list(&json!({ "event": "property-change", "name": "volume", "data": 3 })), None);
    }

    #[test]
    fn picks_the_chosen_device_only_when_connected() {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let out = Output::new("wasapi/{tv}");
        out.attach(tx);
        let dev = |id: &str| AudioDevice { id: id.into(), name: id.into() };
        let sent = |rx: &mut mpsc::UnboundedReceiver<String>| {
            std::iter::from_fn(|| rx.try_recv().ok())
                .map(|m| serde_json::from_str::<Value>(&m).unwrap()["command"][2].as_str().unwrap().to_string())
                .collect::<Vec<_>>()
        };

        // Antes de saber que hay conectado no se toca nada.
        out.select("wasapi/{tv}");
        assert!(sent(&mut rx).is_empty());
        // La tele esta apagada: se queda en la de Windows.
        out.set_devices(vec![dev("wasapi/{auris}")]);
        assert!(sent(&mut rx).is_empty());
        // Se prende la tele: pasa a la tele, una sola vez.
        out.set_devices(vec![dev("wasapi/{auris}"), dev("wasapi/{tv}")]);
        out.set_devices(vec![dev("wasapi/{auris}"), dev("wasapi/{tv}")]);
        assert_eq!(sent(&mut rx), ["wasapi/{tv}"]);
        // Se apaga: vuelve a la de Windows.
        out.set_devices(vec![dev("wasapi/{auris}")]);
        assert_eq!(sent(&mut rx), ["auto"]);
        out.select("wasapi/{auris}");
        out.select("");
        assert_eq!(sent(&mut rx), ["wasapi/{auris}", "auto"]);
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
