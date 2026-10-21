//! Haze para Android. La cola y el reproductor son los de `haze-core`, igual que en
//! escritorio; el motor es Media3 (plugin `haze-player`) y la biblioteca sale de
//! MediaStore. Home Assistant usa el mismo puente MQTT que escritorio y YouTube, el
//! mismo yt-dlp (con `youtubedl-android`).

mod updater;
mod youtube;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, RwLock};
use std::time::Duration;

use haze_core::mqtt::{self, ALL_SONGS};
use haze_core::player::{Listener, Player, PlayerState, QueueView, SharedPlayer};
use haze_core::settings::Settings;
use haze_core::song::{unique_files, Song};
use serde::{Deserialize, Serialize};
use serde_json::json;
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_haze_player::{HazePlayerExt, PlayerEvent};
use tokio::sync::{mpsc, watch};

#[derive(Default)]
struct Library {
    songs: Vec<Arc<Song>>,
    by_id: HashMap<u64, Arc<Song>>,
}

impl Library {
    fn get(&self, id: u64) -> Option<Arc<Song>> {
        self.by_id.get(&id).cloned()
    }

    fn set(&mut self, songs: Vec<Song>) {
        self.songs = songs.into_iter().map(Arc::new).collect();
        self.by_id = self.songs.iter().map(|s| (s.id, s.clone())).collect();
    }
}

struct AppState {
    settings: Mutex<Settings>,
    settings_path: PathBuf,
    library: RwLock<Library>,
    player: SharedPlayer,
    player_rx: watch::Receiver<PlayerState>,
    mqtt: Mutex<mqtt::Bridge>,
    /// "off", "connected", "error" o "network".
    ha_status: Mutex<String>,
    downloads: Mutex<youtube::Downloads>,
    /// Una version nueva bajandose o lista para instalar.
    update: Mutex<Option<updater::UpdateInfo>>,
    /// Hay permiso para leer la musica.
    access: Mutex<bool>,
    scanning: Mutex<bool>,
    /// El motor ya esta conectado y la sesion anterior retomada.
    ready: Mutex<bool>,
}

/// Manda el estado y la posicion a la pantalla.
struct Screen(AppHandle);

impl Listener for Screen {
    fn state(&self, state: &PlayerState) {
        let _ = self.0.emit("player://state", state);
    }

    fn position(&self, position: f64, duration: f64) {
        let _ = self.0.emit("player://position", json!({ "position": position, "duration": duration }));
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", content = "value", rename_all = "camelCase")]
enum Control {
    Toggle,
    Play,
    Pause,
    Next,
    Previous,
    Seek(f64),
    Volume(f64),
    Shuffle(bool),
    Repeat(String),
    /// La ruta de la carpeta, o "Todas las canciones".
    PlayFolder(String),
    /// Salta a la entrada de la cola con ese uid (desde HA).
    JumpUid(u64),
}

// --------------------------------------------------------------------- comandos

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Bootstrap {
    settings: Settings,
    songs: Vec<Arc<Song>>,
    state: PlayerState,
    position: f64,
    access: bool,
    scanning: bool,
    ready: bool,
    version: String,
    ha_status: String,
    downloads: Vec<youtube::DownloadJob>,
    update: Option<updater::UpdateInfo>,
}

#[tauri::command]
async fn bootstrap(app: AppHandle) -> Bootstrap {
    let state = app.state::<AppState>();
    let (pstate, position) = {
        let p = state.player.lock().await;
        (p.state(), p.position())
    };
    let boot = Bootstrap {
        settings: state.settings.lock().unwrap().clone(),
        songs: state.library.read().unwrap().songs.clone(),
        state: pstate,
        position,
        access: *state.access.lock().unwrap(),
        scanning: *state.scanning.lock().unwrap(),
        ready: *state.ready.lock().unwrap(),
        version: app.package_info().version.to_string(),
        ha_status: state.ha_status.lock().unwrap().clone(),
        downloads: state.downloads.lock().unwrap().jobs.clone(),
        update: state.update.lock().unwrap().clone(),
    };
    boot
}

/// Pide permiso para leer la musica y, si lo dan, la carga.
#[tauri::command]
async fn request_access(app: AppHandle) -> bool {
    let h = app.clone();
    let granted = tauri::async_runtime::spawn_blocking(move || h.haze_player().access(true))
        .await
        .ok()
        .and_then(Result::ok)
        .unwrap_or(false);
    let was = std::mem::replace(&mut *app.state::<AppState>().access.lock().unwrap(), granted);
    if granted && !was {
        load_library(&app).await;
        restore(&app).await;
    }
    granted
}

#[tauri::command]
async fn rescan(app: AppHandle) {
    load_library(&app).await;
}

#[tauri::command]
async fn play_ids(app: AppHandle, ids: Vec<u64>, start: usize, context: String) {
    let state = app.state::<AppState>();
    let songs: Vec<_> = {
        let lib = state.library.read().unwrap();
        ids.iter().filter_map(|id| lib.get(*id)).collect()
    };
    state.player.lock().await.play_context(songs, start, context);
}

/// Las de la biblioteca se usan tal cual; las de YouTube llegan enteras de la pantalla.
fn resolve(app: &AppHandle, songs: Vec<Song>) -> Vec<Arc<Song>> {
    let state = app.state::<AppState>();
    let lib = state.library.read().unwrap();
    songs.into_iter().map(|s| lib.get(s.id).unwrap_or_else(|| Arc::new(s))).collect()
}

#[tauri::command]
async fn play_songs(app: AppHandle, songs: Vec<Song>, start: usize, context: String) {
    let songs = resolve(&app, songs);
    app.state::<AppState>().player.lock().await.play_context(songs, start, context);
}

#[tauri::command]
async fn player_control(app: AppHandle, control: Control) {
    run_control(&app, control).await;
}

/// Lo que piden la pantalla y Home Assistant.
async fn run_control(app: &AppHandle, control: Control) {
    let state = app.state::<AppState>();
    let folder_songs = match &control {
        Control::PlayFolder(path) => Some(folder_songs(&state.library.read().unwrap(), path)),
        _ => None,
    };
    let mut p = state.player.lock().await;
    match control {
        Control::Toggle => p.toggle(),
        Control::Play => p.set_paused(false),
        Control::Pause => p.set_paused(true),
        Control::Next => p.next(),
        Control::Previous => p.previous(),
        Control::Seek(s) => p.seek(s),
        Control::Volume(v) => p.set_volume(v),
        Control::Shuffle(on) => {
            p.set_shuffle(on);
            save_playback(app, &p.state());
        }
        Control::Repeat(mode) => {
            p.set_repeat(mode);
            save_playback(app, &p.state());
        }
        Control::PlayFolder(path) => {
            let name = if path == ALL_SONGS { path } else { path.rsplit('/').next().unwrap_or(&path).to_string() };
            p.play_context(folder_songs.unwrap_or_default(), 0, name);
        }
        Control::JumpUid(uid) => p.jump_uid(uid),
    }
}

/// Las canciones de una carpeta (o todas), por titulo, como las muestra la pantalla.
fn folder_songs(lib: &Library, path: &str) -> Vec<Arc<Song>> {
    let mut songs: Vec<_> = if path == ALL_SONGS {
        unique_files(&lib.songs)
    } else {
        lib.songs.iter().filter(|s| s.folder == path).cloned().collect()
    };
    songs.sort_by_cached_key(|s| s.title.to_lowercase());
    songs
}

/// Aleatorio y repetir se recuerdan para la proxima vez.
fn save_playback(app: &AppHandle, s: &PlayerState) {
    let state = app.state::<AppState>();
    let mut settings = state.settings.lock().unwrap();
    settings.playback.shuffle = s.shuffle;
    settings.playback.repeat = s.repeat.clone();
    settings.save(&state.settings_path);
}

#[tauri::command]
async fn enqueue(app: AppHandle, song: Song, next: bool) {
    let song = resolve(&app, vec![song]).remove(0);
    let state = app.state::<AppState>();
    let mut p = state.player.lock().await;
    if next {
        p.play_next(song);
    } else {
        p.add_to_queue(song);
    }
}

/// Toca un tema suelto (por ejemplo, de una busqueda): suena ya y despues sigue la cola
/// que habia, en vez de reemplazarla.
#[tauri::command]
async fn play_now(app: AppHandle, song: Song) {
    let song = resolve(&app, vec![song]).remove(0);
    app.state::<AppState>().player.lock().await.play_now(song);
}

#[tauri::command]
async fn queue_get(app: AppHandle) -> QueueView {
    app.state::<AppState>().player.lock().await.queue_view()
}

#[tauri::command]
async fn queue_action(app: AppHandle, action: String, index: usize, to: Option<usize>) {
    let state = app.state::<AppState>();
    let mut p = state.player.lock().await;
    match action.as_str() {
        "jump" => p.jump(index),
        "remove" => p.remove(index),
        "move" => p.move_entry(index, to.unwrap_or(index)),
        "clear" => p.clear_queue(),
        _ => {}
    }
}

#[tauri::command]
async fn settings_set(app: AppHandle, settings: Settings) {
    let (ha_changed, hidden_changed) = {
        let state = app.state::<AppState>();
        let mut cur = state.settings.lock().unwrap();
        let mut new = settings;
        // Aleatorio y repetir los maneja el reproductor: la copia de la pantalla puede
        // estar vieja.
        new.playback.shuffle = cur.playback.shuffle;
        new.playback.repeat = cur.playback.repeat.clone();
        let changed = (new.ha != cur.ha, new.hidden_folders != cur.hidden_folders);
        *cur = new;
        cur.save(&state.settings_path);
        changed
    };
    if hidden_changed {
        load_library(&app).await;
    }
    if ha_changed {
        start_mqtt(&app).await;
    }
}

/// "Atras" en la pantalla de inicio: la app queda en segundo plano en vez de cerrarse.
#[tauri::command]
async fn minimize(app: AppHandle) {
    let h = app.clone();
    let _ = tauri::async_runtime::spawn_blocking(move || h.haze_player().minimize()).await;
}

/// Pide el permiso de red local (Android 17) y, si lo dan, reconecta con HA.
#[tauri::command]
async fn request_local_network(app: AppHandle) -> bool {
    let granted = local_network(&app, true).await;
    if granted {
        start_mqtt(&app).await;
    }
    granted
}

async fn local_network(app: &AppHandle, request: bool) -> bool {
    let h = app.clone();
    tauri::async_runtime::spawn_blocking(move || h.haze_player().local_network(request))
        .await
        .ok()
        .and_then(Result::ok)
        .unwrap_or(false)
}

// --------------------------------------------------------------------- home assistant

/// Home Assistant le habla al reproductor del telefono. Vive lo que vive el proceso: con
/// musica sonando lo sostiene el servicio en primer plano; si Android lo mata, el broker
/// marca a Haze como desconectado (last will).
struct Ha(AppHandle);

impl mqtt::Hooks for Ha {
    fn folders(&self) -> Vec<(String, String)> {
        let state = self.0.state::<AppState>();
        let lib = state.library.read().unwrap();
        mqtt::folder_options(lib.songs.iter().map(|s| s.folder.as_str()))
    }

    fn command(&self, c: mqtt::Command) {
        use mqtt::Command as C;
        let control = match c {
            C::Toggle => Control::Toggle,
            C::Play => Control::Play,
            C::Pause => Control::Pause,
            C::Next => Control::Next,
            C::Previous => Control::Previous,
            C::Seek(s) => Control::Seek(s),
            C::Volume(v) => Control::Volume(v),
            C::Shuffle(on) => Control::Shuffle(on),
            C::Repeat(m) => Control::Repeat(m),
            C::PlayFolder(p) => Control::PlayFolder(p),
            C::Jump(uid) => Control::JumpUid(uid),
        };
        let app = self.0.clone();
        tauri::async_runtime::spawn(async move { run_control(&app, control).await });
    }

    fn status(&self, s: &str) {
        *self.0.state::<AppState>().ha_status.lock().unwrap() = s.to_string();
        let _ = self.0.emit("ha://status", s);
    }

    fn cover(&self, song: &Song) -> Option<Vec<u8>> {
        self.0.haze_player().cover(&song.path, 512).ok().flatten()
    }

    fn queue(&self) -> QueueView {
        self.0.state::<AppState>().player.blocking_lock().queue_view()
    }
}

async fn start_mqtt(app: &AppHandle) {
    let state = app.state::<AppState>();
    let cfg = state.settings.lock().unwrap().ha.clone();
    // Sin el permiso, Android corta la conexion y solo se veria un timeout.
    if cfg.enabled && !local_network(app, false).await {
        state.mqtt.lock().unwrap().stop();
        mqtt::Hooks::status(&Ha(app.clone()), "network");
        return;
    }
    let device = mqtt::Device { model: "Haze (Android)", version: app.package_info().version.to_string() };
    state.mqtt.lock().unwrap().restart(
        tauri::async_runtime::handle().inner(),
        cfg,
        device,
        state.player_rx.clone(),
        Arc::new(Ha(app.clone())),
    );
}

// --------------------------------------------------------------------- arranque

async fn load_library(app: &AppHandle) {
    let state = app.state::<AppState>();
    *state.scanning.lock().unwrap() = true;
    let _ = app.emit("library://scanning", true);
    let h = app.clone();
    let found = tauri::async_runtime::spawn_blocking(move || h.haze_player().library()).await;
    match found {
        Ok(Ok(tracks)) => {
            let hidden = state.settings.lock().unwrap().hidden_folders.clone();
            let songs: Vec<Song> = tracks
                .into_iter()
                .map(|t| t.into_song())
                .filter(|s| !hidden.contains(&s.folder))
                .collect();
            let mut lib = state.library.write().unwrap();
            lib.set(songs);
            let _ = app.emit("library://changed", &lib.songs);
        }
        Ok(Err(e)) => {
            let _ = app.emit("app://error", format!("No se pudo leer la musica: {e}"));
        }
        Err(e) => log::error!("haze: la lectura de la biblioteca se corto: {e}"),
    }
    *state.scanning.lock().unwrap() = false;
    let _ = app.emit("library://scanning", false);
}

/// Retoma la sesion anterior, en pausa, si los ajustes lo piden.
async fn restore(app: &AppHandle) {
    let state = app.state::<AppState>();
    if !state.settings.lock().unwrap().playback.resume {
        return;
    }
    let mut p = state.player.lock().await;
    if p.state().song.is_some() {
        return;
    }
    let lib = state.library.read().unwrap();
    p.restore(|id| lib.get(id));
}

/// Conecta el motor, carga la biblioteca y retoma la sesion. Corre una vez, de fondo.
async fn start(app: AppHandle) {
    let state = app.state::<AppState>();

    // Lo que avisa Android llega en su hilo principal: se pasa a un canal y lo procesa
    // una tarea, en orden, del lado del reproductor.
    let (tx, mut rx) = mpsc::unbounded_channel::<PlayerEvent>();
    let h = app.clone();
    let listening = tauri::async_runtime::spawn_blocking(move || {
        h.haze_player().listen(move |ev| {
            let _ = tx.send(ev);
        })
    })
    .await;
    let error = match listening {
        Ok(Ok(())) => None,
        Ok(Err(e)) => Some(e.to_string()),
        Err(e) => Some(e.to_string()),
    };
    if let Some(e) = error {
        let _ = app.emit("app://error", format!("No se pudo preparar el reproductor: {e}"));
        return;
    }
    state.player.lock().await.attach(app.haze_player().engine());

    let player = state.player.clone();
    let h = app.clone();
    tauri::async_runtime::spawn(async move {
        // Cada tanto se guarda la sesion: Android puede matar la app sin avisar.
        let mut save = tokio::time::interval(Duration::from_secs(15));
        loop {
            tokio::select! {
                ev = rx.recv() => {
                    let Some(ev) = ev else { return };
                    match &ev {
                        PlayerEvent::Download { id, line } => {
                            youtube::on_line(&h, *id, line);
                            continue;
                        }
                        PlayerEvent::UpdateProgress(p) => {
                            updater::on_progress(&h, *p);
                            continue;
                        }
                        _ => {}
                    }
                    let mut p = player.lock().await;
                    match ev {
                        PlayerEvent::Engine(e) => p.on_event(e),
                        PlayerEvent::Next => p.next(),
                        PlayerEvent::Previous => p.previous(),
                        PlayerEvent::Download { .. } | PlayerEvent::UpdateProgress(_) => {}
                    }
                }
                _ = save.tick() => {
                    let p = player.lock().await;
                    if p.state().playing {
                        p.save_session();
                    }
                }
            }
        }
    });

    let h = app.clone();
    let granted = tauri::async_runtime::spawn_blocking(move || h.haze_player().access(false))
        .await
        .ok()
        .and_then(Result::ok)
        .unwrap_or(false);
    *state.access.lock().unwrap() = granted;
    if granted {
        load_library(&app).await;
        restore(&app).await;
    }
    *state.ready.lock().unwrap() = true;
    let _ = app.emit("app://ready", granted);
    // Despues de la biblioteca: asi el primer anuncio ya trae las carpetas.
    start_mqtt(&app).await;
    tauri::async_runtime::spawn(updater::run(app.clone()));
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_haze_player::init())
        .register_asynchronous_uri_scheme_protocol("cover", |ctx, req, responder| {
            let app = ctx.app_handle().clone();
            let id: u64 = req.uri().path().trim_start_matches('/').parse().unwrap_or(0);
            std::thread::spawn(move || {
                let song = app.state::<AppState>().library.read().unwrap().get(id);
                let bytes = song.and_then(|s| app.haze_player().cover(&s.path, 320).ok().flatten());
                let resp = match bytes {
                    Some(b) => tauri::http::Response::builder()
                        .header("Content-Type", "image/jpeg")
                        .header("Cache-Control", "max-age=86400")
                        .header("Access-Control-Allow-Origin", "*")
                        .body(b),
                    None => tauri::http::Response::builder().status(404).body(Vec::new()),
                };
                responder.respond(resp.unwrap());
            });
        })
        .setup(|app| {
            let handle = app.handle().clone();
            let config_dir = app.path().app_config_dir()?;
            let data_dir = app.path().app_local_data_dir()?;
            std::fs::create_dir_all(&config_dir)?;
            std::fs::create_dir_all(&data_dir)?;

            let settings_path = config_dir.join("settings.json");
            let fresh = !settings_path.exists();
            let mut settings = Settings::load(&settings_path);
            if fresh {
                // En el telefono la lista arranca por lo ultimo que entro (lo recien bajado).
                settings.song_sort = "date_desc".into();
            }
            // En el telefono el volumen es el del sistema: Haze suena siempre entero.
            let (player, player_rx) = Player::new(
                Screen(handle.clone()),
                100.0,
                settings.playback.shuffle,
                settings.playback.repeat.clone(),
                data_dir.join("session.json"),
            );

            app.manage(AppState {
                settings: Mutex::new(settings),
                settings_path,
                library: RwLock::new(Library::default()),
                player: Arc::new(tokio::sync::Mutex::new(player)),
                player_rx,
                mqtt: Mutex::new(mqtt::Bridge::new()),
                ha_status: Mutex::new("off".into()),
                downloads: Mutex::new(Default::default()),
                update: Mutex::new(None),
                access: Mutex::new(false),
                scanning: Mutex::new(false),
                ready: Mutex::new(false),
            });

            tauri::async_runtime::spawn(start(handle));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            bootstrap,
            request_access,
            rescan,
            play_ids,
            play_songs,
            player_control,
            enqueue,
            play_now,
            queue_get,
            queue_action,
            settings_set,
            request_local_network,
            minimize,
            youtube::yt_search,
            youtube::yt_inspect,
            youtube::yt_download,
            youtube::downloads_clear,
            updater::update_check,
            updater::update_install,
        ])
        .build(tauri::generate_context!())
        .expect("no se pudo arrancar Haze")
        .run(|app, event| match event {
            // Sacar Haze de recientes destruye la Activity y Tauri, sin ventanas, cerraba el
            // proceso y se cortaba la musica. El proceso sigue mientras lo sostenga el
            // servicio de reproduccion.
            tauri::RunEvent::ExitRequested { api, code: None, .. } => api.prevent_exit(),
            // Al abrir la app de nuevo hay una Activity nueva sin ventana: se le crea la
            // de siempre. Hasta que exista, `build` falla y se reintenta en la proxima vuelta.
            tauri::RunEvent::MainEventsCleared if app.get_webview_window("main").is_none() => {
                if let Some(config) = app.config().app.windows.first() {
                    if let Ok(builder) = tauri::WebviewWindowBuilder::from_config(app, config) {
                        let _ = builder.build();
                    }
                }
            }
            _ => {}
        });
}
