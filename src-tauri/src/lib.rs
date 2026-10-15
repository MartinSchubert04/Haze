//! Reproductor de escritorio. Rust hace todo lo pesado (biblioteca, cola, mpv, YouTube,
//! Home Assistant); la interfaz es una vista que se puede cerrar sin cortar la musica.
//!
//! Al cerrar la ventana se destruye el WebView entero: de fondo queda solo este proceso
//! y mpv, que es lo que lo hace liviano.

mod library;
mod media_keys;
mod mpv;
mod mqtt;
mod player;
mod queue;
mod settings;
mod tools;
mod youtube;

use std::path::PathBuf;
use std::sync::{Arc, Mutex, RwLock};

use serde::Deserialize;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::window::{Effect, EffectsBuilder};
use tauri::{AppHandle, Emitter, Manager, RunEvent, WebviewUrl, WebviewWindowBuilder, WindowEvent};
use tokio::sync::watch;

use library::{Library, Song};
use player::{Player, PlayerState, SharedPlayer};
use settings::Settings;

pub struct AppState {
    pub settings: Mutex<Settings>,
    pub settings_path: PathBuf,
    pub library: RwLock<Library>,
    pub cache_path: PathBuf,
    pub player: SharedPlayer,
    pub player_rx: watch::Receiver<PlayerState>,
    pub downloads: Mutex<youtube::Downloads>,
    pub mqtt: Mutex<mqtt::Bridge>,
    pub ha_status: Mutex<String>,
    pub scanning: Mutex<bool>,
}

/// Ordenes que llegan de afuera de la interfaz: bandeja, teclas multimedia, HA.
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", content = "value", rename_all = "camelCase")]
pub enum Control {
    Toggle,
    Play,
    Pause,
    Next,
    Previous,
    Seek(f64),
    Volume(f64),
    Shuffle(bool),
    Repeat(String),
    PlayFolder(String),
}

pub const ALL_SONGS: &str = "Todas las canciones";

fn songs_sorted(mut songs: Vec<Arc<Song>>) -> Vec<Arc<Song>> {
    songs.sort_by_cached_key(|s| s.title.to_lowercase());
    songs
}

pub fn control(app: &AppHandle, c: Control) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let state = app.state::<AppState>();
        let folder_songs = if let Control::PlayFolder(path) = &c {
            let lib = state.library.read().unwrap();
            let songs: Vec<_> = if path == ALL_SONGS {
                lib.songs.clone()
            } else {
                lib.songs.iter().filter(|s| &s.folder == path).cloned().collect()
            };
            Some(songs_sorted(songs))
        } else {
            None
        };
        let mut p = state.player.lock().await;
        match c {
            Control::Toggle => p.toggle(),
            Control::Play => p.set_paused(false),
            Control::Pause => p.set_paused(true),
            Control::Next => p.next(),
            Control::Previous => p.previous(),
            Control::Seek(s) => p.seek(s),
            Control::Volume(v) => p.set_volume(v),
            Control::Shuffle(on) => p.set_shuffle(on),
            Control::Repeat(m) => p.set_repeat(m),
            Control::PlayFolder(path) => {
                let name = if path == ALL_SONGS { path.clone() } else { folder_name(&path) };
                p.play_context(folder_songs.unwrap_or_default(), 0, name);
            }
        }
    });
}

fn folder_name(path: &str) -> String {
    path.rsplit('/').next().unwrap_or(path).to_string()
}

/// Carpetas para el selector de HA: (nombre visible, ruta).
fn folder_options(app: &AppHandle) -> Vec<(String, String)> {
    let state = app.state::<AppState>();
    let lib = state.library.read().unwrap();
    let mut paths: Vec<String> = lib.songs.iter().map(|s| s.folder.clone()).collect();
    paths.sort();
    paths.dedup();
    let mut out = vec![(ALL_SONGS.to_string(), ALL_SONGS.to_string())];
    let mut used = std::collections::HashSet::new();
    for p in paths {
        let mut name = folder_name(&p);
        // Dos carpetas con el mismo nombre: se distinguen por la de arriba.
        if !used.insert(name.to_lowercase()) {
            let parts: Vec<&str> = p.rsplit('/').take(2).collect();
            name = format!("{} / {}", parts.get(1).unwrap_or(&""), parts[0]);
            used.insert(name.to_lowercase());
        }
        out.push((name, p));
    }
    out.truncate(150);
    out
}

// --------------------------------------------------------------------- comandos

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct Bootstrap {
    settings: Settings,
    songs: Vec<Arc<Song>>,
    state: PlayerState,
    position: f64,
    /// mpv ya esta andando.
    engine: bool,
    scanning: bool,
    ha_status: String,
    downloads: Vec<youtube::DownloadJob>,
    version: String,
}

#[tauri::command]
async fn bootstrap(app: AppHandle) -> Bootstrap {
    let state = app.state::<AppState>();
    let (pstate, position, engine) = {
        let p = state.player.lock().await;
        (p.state(), p.position(), p.has_engine())
    };
    let settings = state.settings.lock().unwrap().clone();
    let songs = state.library.read().unwrap().songs.clone();
    let scanning = *state.scanning.lock().unwrap();
    let ha_status = state.ha_status.lock().unwrap().clone();
    let downloads = state.downloads.lock().unwrap().jobs.clone();
    Bootstrap {
        settings,
        songs,
        state: pstate,
        position,
        engine,
        scanning,
        ha_status,
        downloads,
        version: env!("CARGO_PKG_VERSION").into(),
    }
}

#[tauri::command]
fn rescan(app: AppHandle) {
    start_scan(&app);
}

fn resolve(app: &AppHandle, songs: Vec<Song>) -> Vec<Arc<Song>> {
    let state = app.state::<AppState>();
    let lib = state.library.read().unwrap();
    songs.into_iter().map(|s| lib.get(s.id).unwrap_or_else(|| Arc::new(s))).collect()
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

#[tauri::command]
async fn play_songs(app: AppHandle, songs: Vec<Song>, start: usize, context: String) {
    let songs = resolve(&app, songs);
    let state = app.state::<AppState>();
    state.player.lock().await.play_context(songs, start, context);
}

#[tauri::command]
fn player_control(app: AppHandle, control: Control) {
    self::control(&app, control);
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

#[tauri::command]
async fn queue_get(app: AppHandle) -> player::QueueView {
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
fn settings_set(app: AppHandle, settings: Settings) {
    let state = app.state::<AppState>();
    let (sources_changed, ha_changed, bg_changed) = {
        let mut cur = state.settings.lock().unwrap();
        let mut new = settings;
        // Lo que se baja tiene que aparecer en la biblioteca: las carpetas de descarga
        // cuentan como fuente aunque el usuario no las haya agregado ahi.
        for d in &new.download_folders {
            let inside = new.sources.iter().any(|s| d.to_lowercase().starts_with(&s.to_lowercase()));
            if !inside {
                new.sources.push(d.clone());
            }
        }
        // Lo de reproduccion lo maneja el reproductor, no la pantalla de ajustes.
        new.playback = cur.playback.clone();
        let changes = (
            new.sources != cur.sources,
            serde_json::to_string(&new.ha).ok() != serde_json::to_string(&cur.ha).ok(),
            new.appearance.background != cur.appearance.background,
        );
        *cur = new;
        cur.save(&state.settings_path);
        changes
    };
    let s = state.settings.lock().unwrap().clone();
    let _ = app.emit("settings://changed", &s);
    if sources_changed {
        start_scan(&app);
    }
    if ha_changed {
        start_mqtt(&app);
    }
    if bg_changed {
        apply_effects(&app);
    }
}

#[tauri::command]
async fn yt_search(app: AppHandle, query: String) -> Result<Vec<Song>, String> {
    youtube::search(&app, &query).await
}

#[tauri::command]
async fn yt_inspect(app: AppHandle, url: String) -> Result<youtube::LinkInfo, String> {
    youtube::inspect(&app, &url).await
}

#[tauri::command]
async fn yt_prepare(app: AppHandle) -> Result<(), String> {
    // Escuchar YouTube necesita yt-dlp y deno; se bajan antes de la primera vez.
    youtube::ensure_tool(&app, tools::Tool::YtDlp).await?;
    youtube::ensure_tool(&app, tools::Tool::Deno).await?;
    Ok(())
}

#[tauri::command]
fn yt_download(app: AppHandle, url: String, title: String, thumb: Option<String>, folder: String, playlist: bool) -> u64 {
    youtube::enqueue(&app, url, title, thumb, folder, playlist)
}

#[tauri::command]
fn downloads_clear(app: AppHandle) {
    youtube::clear_finished(&app);
}

// Async a proposito: crear una ventana desde un comando sincronico traba Windows.
#[tauri::command]
async fn mini_open(app: AppHandle) {
    open_mini(&app);
}

#[tauri::command]
async fn main_show(app: AppHandle) {
    show_main(&app);
}

#[tauri::command]
fn quit(app: AppHandle) {
    shutdown(&app);
    app.exit(0);
}

#[tauri::command]
fn mini_moved(app: AppHandle, x: i32, y: i32) {
    let state = app.state::<AppState>();
    let mut s = state.settings.lock().unwrap();
    s.behavior.mini_position = Some((x, y));
    s.save(&state.settings_path);
}

// --------------------------------------------------------------------- ventanas

fn glass_effects(app: &AppHandle) -> Option<tauri::utils::config::WindowEffectsConfig> {
    let bg = app.state::<AppState>().settings.lock().unwrap().appearance.background.clone();
    // Con imagen o color liso el vidrio de Windows no se ve: no se paga su costo.
    (bg == "glass").then(|| EffectsBuilder::new().effect(Effect::Acrylic).build())
}

fn apply_effects(app: &AppHandle) {
    let fx = glass_effects(app);
    for w in app.webview_windows().values() {
        let _ = w.set_effects(fx.clone());
    }
}

/// Para depurar sin pantalla: `REPRODUCTOR_DEVTOOLS_PORT=9333` abre el protocolo de
/// DevTools del WebView en ese puerto. Hay que repetir los argumentos que Tauri pone
/// por defecto, porque este los reemplaza.
fn browser_args() -> Option<String> {
    let port = std::env::var("REPRODUCTOR_DEVTOOLS_PORT").ok()?;
    Some(format!(
        "--disable-features=msWebOOUI,msPdfOOUI,msSmartScreenProtection --remote-debugging-port={port}"
    ))
}

pub fn show_main(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
        return;
    }
    let mut b = WebviewWindowBuilder::new(app, "main", WebviewUrl::App("index.html".into()))
        .title("Reproductor")
        .inner_size(1120.0, 720.0)
        .min_inner_size(780.0, 520.0)
        .decorations(false)
        .transparent(true)
        .shadow(true)
        .visible(false)
        .center();
    if let Some(args) = browser_args() {
        b = b.additional_browser_args(&args);
    }
    if let Some(fx) = glass_effects(app) {
        b = b.effects(fx);
    }
    if let Ok(w) = b.build() {
        let app2 = app.clone();
        w.on_window_event(move |e| {
            if let WindowEvent::CloseRequested { .. } = e {
                let to_tray = app2.state::<AppState>().settings.lock().unwrap().behavior.close_to_tray;
                if !to_tray {
                    shutdown(&app2);
                    app2.exit(0);
                }
            }
        });
    }
}

pub fn open_mini(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("mini") {
        let _ = w.show();
        let _ = w.set_focus();
        return;
    }
    let (on_top, pos) = {
        let s = app.state::<AppState>().settings.lock().unwrap().behavior.clone();
        (s.mini_on_top, s.mini_position)
    };
    let mut b = WebviewWindowBuilder::new(app, "mini", WebviewUrl::App("index.html#mini".into()))
        .title("Reproductor - mini")
        .inner_size(380.0, 96.0)
        .resizable(false)
        .maximizable(false)
        .minimizable(false)
        .decorations(false)
        .transparent(true)
        .shadow(true)
        .always_on_top(on_top)
        .skip_taskbar(true)
        .visible(false);
    if let Some(args) = browser_args() {
        b = b.additional_browser_args(&args);
    }
    if let Some(fx) = glass_effects(app) {
        b = b.effects(fx);
    }
    b = match pos {
        Some((x, y)) => b.position(x as f64, y as f64),
        None => b.center(),
    };
    let _ = b.build();
}

// --------------------------------------------------------------------- bandeja

fn build_tray(app: &AppHandle) -> tauri::Result<()> {
    let toggle = MenuItem::with_id(app, "toggle", "Reproducir / Pausar", true, None::<&str>)?;
    let next = MenuItem::with_id(app, "next", "Siguiente", true, None::<&str>)?;
    let prev = MenuItem::with_id(app, "prev", "Anterior", true, None::<&str>)?;
    let mini = MenuItem::with_id(app, "mini", "Mini reproductor", true, None::<&str>)?;
    let open = MenuItem::with_id(app, "open", "Abrir", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Salir", true, None::<&str>)?;
    let sep1 = PredefinedMenuItem::separator(app)?;
    let sep2 = PredefinedMenuItem::separator(app)?;
    let menu = Menu::with_items(app, &[&toggle, &prev, &next, &sep1, &mini, &open, &sep2, &quit])?;

    let tray = TrayIconBuilder::with_id("main")
        .icon(app.default_window_icon().cloned().unwrap())
        .tooltip("Reproductor")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, e| match e.id.as_ref() {
            "toggle" => control(app, Control::Toggle),
            "next" => control(app, Control::Next),
            "prev" => control(app, Control::Previous),
            "mini" => open_mini(app),
            "open" => show_main(app),
            "quit" => {
                shutdown(app);
                app.exit(0);
            }
            _ => {}
        })
        .on_tray_icon_event(|tray, e| {
            if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = e {
                show_main(tray.app_handle());
            }
        })
        .build(app)?;

    // El tooltip muestra lo que suena.
    let mut rx = app.state::<AppState>().player_rx.clone();
    tauri::async_runtime::spawn(async move {
        while rx.changed().await.is_ok() {
            let s = rx.borrow_and_update().clone();
            let tip = match &s.song {
                Some(song) if song.artist.is_empty() => song.title.clone(),
                Some(song) => format!("{} - {}", song.title, song.artist),
                None => "Reproductor".into(),
            };
            // Windows corta el tooltip en 127 caracteres.
            let tip: String = tip.chars().take(120).collect();
            let _ = tray.set_tooltip(Some(&tip));
            let _ = toggle.set_text(if s.playing { "Pausar" } else { "Reproducir" });
        }
    });
    Ok(())
}

// --------------------------------------------------------------------- arranque

fn start_scan(app: &AppHandle) {
    let state = app.state::<AppState>();
    {
        let mut s = state.scanning.lock().unwrap();
        if *s {
            return;
        }
        *s = true;
    }
    let _ = app.emit("library://scanning", true);
    let app = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        let sources = state.settings.lock().unwrap().sources.clone();
        let cached: Vec<Song> = state.library.read().unwrap().songs.iter().map(|s| (**s).clone()).collect();
        let songs = library::scan(&sources, &cached);
        {
            let mut lib = state.library.write().unwrap();
            lib.set(songs);
            lib.save_cache(&state.cache_path);
        }
        *state.scanning.lock().unwrap() = false;
        let songs = state.library.read().unwrap().songs.clone();
        let _ = app.emit("library://changed", songs);
        let _ = app.emit("library://scanning", false);
    });
}

fn start_mqtt(app: &AppHandle) {
    let state = app.state::<AppState>();
    let cfg = state.settings.lock().unwrap().ha.clone();
    let a1 = app.clone();
    let a2 = app.clone();
    let a3 = app.clone();
    state.mqtt.lock().unwrap().restart(
        cfg,
        state.player_rx.clone(),
        move || folder_options(&a1),
        move |c| control(&a2, c),
        move |s| {
            *a3.state::<AppState>().ha_status.lock().unwrap() = s.to_string();
            let _ = a3.emit("ha://status", s);
        },
    );
}

/// Guarda volumen, aleatorio y repetir cuando cambian.
fn persist_playback(app: &AppHandle) {
    let mut rx = app.state::<AppState>().player_rx.clone();
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        while rx.changed().await.is_ok() {
            let s = rx.borrow_and_update().clone();
            let state = app.state::<AppState>();
            let mut set = state.settings.lock().unwrap();
            let pb = &mut set.playback;
            if pb.volume != s.volume || pb.shuffle != s.shuffle || pb.repeat != s.repeat {
                pb.volume = s.volume;
                pb.shuffle = s.shuffle;
                pb.repeat = s.repeat.clone();
                set.save(&state.settings_path);
            }
        }
    });
}

fn shutdown(app: &AppHandle) {
    let state = app.state::<AppState>();
    if let Ok(p) = state.player.try_lock() {
        p.save_session();
    }
    state.mqtt.lock().unwrap().stop();
}

async fn start_engine(app: AppHandle) {
    let state = app.state::<AppState>();
    let mpv_path = match youtube::ensure_tool(&app, tools::Tool::Mpv).await {
        Ok(p) => p,
        Err(e) => {
            let _ = app.emit("app://error", format!("No se pudo preparar el motor de audio: {e}"));
            return;
        }
    };
    let volume = state.settings.lock().unwrap().playback.volume;
    let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
    match mpv::Mpv::spawn(&mpv_path, volume, tx).await {
        Ok(m) => {
            let resume = state.settings.lock().unwrap().playback.resume;
            let mut p = state.player.lock().await;
            p.attach(m);
            if resume {
                let lib = state.library.read().unwrap();
                p.restore(|id| lib.get(id));
            }
        }
        Err(e) => {
            let _ = app.emit("app://error", e);
            return;
        }
    }
    player::pump(state.player.clone(), rx);
    let _ = app.emit("app://ready", ());

    // yt-dlp viejo deja de andar con YouTube: se actualiza de fondo cada tanto.
    let stamp = tools::bin_dir().join("yt-dlp.updated");
    let stale = std::fs::metadata(&stamp)
        .and_then(|m| m.modified())
        .map(|t| t.elapsed().unwrap_or_default().as_secs() > 3 * 24 * 3600)
        .unwrap_or(true);
    if stale {
        tauri::async_runtime::spawn_blocking(move || {
            tools::update_ytdlp();
            let _ = std::fs::write(stamp, b"");
        });
    }
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| show_main(app)))
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            Some(vec!["--hidden"]),
        ))
        .register_asynchronous_uri_scheme_protocol("cover", |ctx, req, responder| {
            let app = ctx.app_handle().clone();
            let id: u64 = req.uri().path().trim_start_matches('/').split('?').next().unwrap_or("").parse().unwrap_or(0);
            std::thread::spawn(move || {
                let song = app.state::<AppState>().library.read().unwrap().get(id);
                let res = song.and_then(|s| library::cover(&s));
                let resp = match res {
                    Some((bytes, mime)) => tauri::http::Response::builder()
                        .header("Content-Type", mime)
                        .header("Cache-Control", "max-age=86400")
                        .header("Access-Control-Allow-Origin", "*")
                        .body(bytes),
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
            tools::init(data_dir.join("bin"));

            let settings_path = config_dir.join("settings.json");
            let settings = Settings::load(&settings_path);
            let cache_path = data_dir.join("library.json");
            let mut library = Library::default();
            library.set(Library::load_cache(&cache_path));

            let (player, player_rx) = Player::new(
                handle.clone(),
                settings.playback.volume,
                settings.playback.shuffle,
                settings.playback.repeat.clone(),
                data_dir.join("session.json"),
            );
            let start_hidden = settings.behavior.start_hidden || std::env::args().any(|a| a == "--hidden");

            app.manage(AppState {
                settings: Mutex::new(settings),
                settings_path,
                library: RwLock::new(library),
                cache_path,
                player: Arc::new(tokio::sync::Mutex::new(player)),
                player_rx,
                downloads: Mutex::new(Default::default()),
                mqtt: Mutex::new(mqtt::Bridge::new()),
                ha_status: Mutex::new("off".into()),
                scanning: Mutex::new(false),
            });

            build_tray(&handle)?;
            let h = handle.clone();
            media_keys::start(app.state::<AppState>().player_rx.clone(), move |c| control(&h, c));
            persist_playback(&handle);
            start_mqtt(&handle);
            start_scan(&handle);
            tauri::async_runtime::spawn(start_engine(handle.clone()));

            if !start_hidden {
                show_main(&handle);
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            bootstrap,
            rescan,
            play_ids,
            play_songs,
            player_control,
            enqueue,
            queue_get,
            queue_action,
            settings_set,
            yt_search,
            yt_inspect,
            yt_prepare,
            yt_download,
            downloads_clear,
            mini_open,
            main_show,
            quit,
            mini_moved,
        ])
        .build(tauri::generate_context!())
        .expect("no se pudo iniciar la app")
        .run(|app, e| {
            // Cerrar la ultima ventana no cierra la app: sigue en la bandeja.
            match e {
                RunEvent::ExitRequested { code: None, api, .. } => api.prevent_exit(),
                RunEvent::Exit => shutdown(app),
                _ => {}
            }
        });
}
