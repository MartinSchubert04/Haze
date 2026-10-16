use std::sync::mpsc;

use base64::Engine as _;
use haze_core::player::Engine;
use haze_core::song::{Song, SongKind};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tauri::ipc::{Channel, InvokeResponseBody};
use tauri::plugin::mobile::PluginInvokeError;
use tauri::plugin::PluginHandle;
use tauri::{Manager, Runtime};

use crate::{parse, PlayerEvent, Release, Track};

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
        // Las ordenes van por JNI a `EngineBridge`, no por el plugin: los comandos del
        // plugin necesitan la Activity, y con la app sacada de recientes la musica sigue y
        // hay que poder pasar a la siguiente. Se mandan en orden desde un hilo propio.
        std::thread::Builder::new()
            .name("haze-player".into())
            .spawn(move || {
                let mut bridge = None;
                for (command, payload) in rx {
                    if let Err(e) = bridge::call(&mut bridge, command, &payload.to_string()) {
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

    /// Si se puede hablar con la red local (el broker de HA). Con `request`, lo pide si
    /// falta.
    pub fn local_network(&self, request: bool) -> Result<bool, PluginInvokeError> {
        #[derive(Deserialize)]
        struct Access {
            granted: bool,
        }
        let command = if request { "requestNetwork" } else { "checkNetwork" };
        Ok(self.call::<Access>(command, ())?.granted)
    }

    pub fn library(&self) -> Result<Vec<Track>, PluginInvokeError> {
        #[derive(Deserialize)]
        struct Library {
            songs: Vec<Track>,
        }
        Ok(self.call::<Library>("library", ())?.songs)
    }

    /// Corre yt-dlp y devuelve lo que imprimio. Tarda: llamar fuera del runtime.
    pub fn yt_run(&self, args: &[&str]) -> Result<String, PluginInvokeError> {
        #[derive(Deserialize)]
        struct Out {
            out: String,
        }
        Ok(self.call::<Out>("ytRun", json!({ "args": args }))?.out)
    }

    /// Baja con yt-dlp y guarda en Music/Haze. El progreso llega como
    /// `PlayerEvent::Download`. Devuelve cuantos archivos guardo.
    pub fn yt_download(&self, id: u64, args: &[&str]) -> Result<usize, PluginInvokeError> {
        #[derive(Deserialize)]
        struct Saved {
            saved: usize,
        }
        Ok(self.call::<Saved>("ytDownload", json!({ "id": id, "args": args }))?.saved)
    }

    /// Manda la app al fondo (como el boton de inicio), sin cerrarla.
    pub fn minimize(&self) -> Result<(), PluginInvokeError> {
        self.call::<Value>("minimize", ())?;
        Ok(())
    }

    /// La ultima version publicada para Android, si hay. Tarda: llamar fuera del runtime.
    pub fn update_latest(&self) -> Result<Option<Release>, PluginInvokeError> {
        let v = self.call::<Value>("updateLatest", ())?;
        Ok(serde_json::from_value(v).ok())
    }

    /// Baja y verifica el APK. El avance llega como `PlayerEvent::UpdateProgress`.
    pub fn update_download(&self, release: &Release) -> Result<(), PluginInvokeError> {
        self.call::<Value>("updateDownload", json!({ "url": release.url, "sha256": release.sha256 }))?;
        Ok(())
    }

    /// Instala lo bajado. `Ok(false)` si antes hay que dar el permiso de instalar apps (se
    /// abrio ese ajuste).
    pub fn update_install(&self) -> Result<bool, PluginInvokeError> {
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Installed {
            needs_permission: bool,
        }
        Ok(!self.call::<Installed>("updateInstall", ())?.needs_permission)
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
                "stream": song.kind == SongKind::Stream,
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

/// `EngineBridge.call(command, json)` en Kotlin, por JNI.
mod bridge {
    use jni::objects::{GlobalRef, JClass, JObject, JValue};
    use jni::JavaVM;

    const CLASS: &str = "app.haze.player.plugin.EngineBridge";

    /// La clase se busca una vez con el class loader de la app (desde un hilo de Rust,
    /// `FindClass` solo ve las del sistema) y se guarda.
    pub fn call(class: &mut Option<GlobalRef>, command: &str, json: &str) -> jni::errors::Result<()> {
        let ctx = ndk_context::android_context();
        let vm = unsafe { JavaVM::from_raw(ctx.vm().cast()) }?;
        let mut env = vm.attach_current_thread_permanently()?;
        if class.is_none() {
            let context = unsafe { JObject::from_raw(ctx.context().cast()) };
            let loader = env.call_method(&context, "getClassLoader", "()Ljava/lang/ClassLoader;", &[])?.l()?;
            let name = env.new_string(CLASS)?;
            let found = env
                .call_method(&loader, "loadClass", "(Ljava/lang/String;)Ljava/lang/Class;", &[JValue::Object(&name)])?
                .l()?;
            *class = Some(env.new_global_ref(found)?);
        }
        let class: &JClass = class.as_ref().unwrap().as_obj().into();
        let command = env.new_string(command)?;
        let json = env.new_string(json)?;
        env.call_static_method(
            class,
            "call",
            "(Ljava/lang/String;Ljava/lang/String;)V",
            &[JValue::Object(&command), JValue::Object(&json)],
        )?;
        Ok(())
    }
}
