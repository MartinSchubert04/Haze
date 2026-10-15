use std::sync::mpsc;

use base64::Engine as _;
use haze_core::player::Engine;
use haze_core::song::Song;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tauri::ipc::{Channel, InvokeResponseBody};
use tauri::plugin::mobile::PluginInvokeError;
use tauri::plugin::PluginHandle;
use tauri::{Manager, Runtime};

use crate::{parse, PlayerEvent, Track};

pub struct HazePlayer<R: Runtime>(PluginHandle<R>);

pub trait HazePlayerExt<R: Runtime> {
    fn haze_player(&self) -> &HazePlayer<R>;
}

impl<R: Runtime, T: Manager<R>> HazePlayerExt<R> for T {
    fn haze_player(&self) -> &HazePlayer<R> {
        self.state::<HazePlayer<R>>().inner()
    }
}

impl<R: Runtime> HazePlayer<R> {
    pub(crate) fn new(handle: PluginHandle<R>) -> Self {
        Self(handle)
    }

    /// Bloquea hasta que Kotlin responde: no llamar desde el hilo principal de Android.
    fn call<T: DeserializeOwned>(&self, command: &str, payload: impl Serialize) -> Result<T, PluginInvokeError> {
        self.0.run_mobile_plugin(command, payload)
    }

    /// El motor para `haze_core::player::Player`.
    pub fn engine(&self) -> AndroidEngine {
        let (tx, rx) = mpsc::channel::<(&'static str, Value)>();
        let handle = self.0.clone();
        // Los comandos esperan la respuesta de Kotlin: se mandan desde un hilo propio,
        // en orden, para que el reproductor nunca quede trabado esperando.
        std::thread::Builder::new()
            .name("haze-player".into())
            .spawn(move || {
                for (command, payload) in rx {
                    if let Err(e) = handle.run_mobile_plugin::<Value>(command, payload) {
                        log::warn!("haze-player: {command} fallo: {e}");
                    }
                }
            })
            .expect("no se pudo crear el hilo de haze-player");
        AndroidEngine { tx }
    }

    /// Recibe lo que pasa en Android. `on_event` corre en el hilo principal de Android:
    /// tiene que volver enseguida (pasarlo a un canal) y nunca llamar al plugin.
    pub fn listen(&self, on_event: impl Fn(PlayerEvent) + Send + Sync + 'static) -> Result<(), PluginInvokeError> {
        let channel: Channel = Channel::new(move |body| {
            if let InvokeResponseBody::Json(json) = body {
                if let Some(ev) = parse(&json) {
                    on_event(ev);
                }
            }
            Ok(())
        });
        self.call::<Value>("listen", json!({ "channel": channel }))?;
        Ok(())
    }

    /// Si hay permiso para leer la musica. Con `request`, lo pide si falta.
    pub fn access(&self, request: bool) -> Result<bool, PluginInvokeError> {
        #[derive(Deserialize)]
        struct Access {
            granted: bool,
        }
        let command = if request { "requestAccess" } else { "checkAccess" };
        Ok(self.call::<Access>(command, ())?.granted)
    }

    pub fn library(&self) -> Result<Vec<Track>, PluginInvokeError> {
        #[derive(Deserialize)]
        struct Library {
            songs: Vec<Track>,
        }
        Ok(self.call::<Library>("library", ())?.songs)
    }

    /// La tapa en JPEG, si el archivo trae una.
    pub fn cover(&self, uri: &str, size: u32) -> Result<Option<Vec<u8>>, PluginInvokeError> {
        #[derive(Deserialize)]
        struct Cover {
            data: Option<String>,
        }
        let cover = self.call::<Cover>("cover", json!({ "uri": uri, "size": size }))?;
        Ok(cover.data.and_then(|d| base64::engine::general_purpose::STANDARD.decode(d).ok()))
    }
}

/// [`Engine`] de Android: cada comando va a Media3 por el plugin, sin esperar.
pub struct AndroidEngine {
    tx: mpsc::Sender<(&'static str, Value)>,
}

impl AndroidEngine {
    fn send(&self, command: &'static str, payload: Value) {
        let _ = self.tx.send((command, payload));
    }
}

impl Engine for AndroidEngine {
    fn load(&self, song: &Song, start: f64, paused: bool) {
        // La notificacion muestra la tapa: la de YouTube por URL o la del archivo, que
        // el servicio lee de MediaStore.
        let artwork = song
            .thumb
            .clone()
            .or_else(|| (song.has_cover && song.path.starts_with("content://")).then(|| song.path.clone()));
        self.send(
            "load",
            json!({
                "uri": song.path,
                "start": start,
                "paused": paused,
                "title": song.title,
                "artist": song.artist,
                "album": song.album,
                "artwork": artwork,
            }),
        );
    }

    fn set_paused(&self, paused: bool) {
        self.send("setPaused", json!({ "paused": paused }));
    }

    fn seek(&self, seconds: f64) {
        self.send("seek", json!({ "seconds": seconds }));
    }

    fn set_volume(&self, volume: f64) {
        self.send("setVolume", json!({ "volume": volume }));
    }

    fn stop(&self) {
        self.send("stop", json!({}));
    }
}
