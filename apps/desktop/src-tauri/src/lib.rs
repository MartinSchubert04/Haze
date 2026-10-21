//! Haze, reproductor de musica de escritorio. Rust hace todo lo pesado (biblioteca, cola, mpv, YouTube,
//! Home Assistant); la interfaz es una vista que se puede cerrar sin cortar la musica.
//!
//! Al cerrar la ventana se destruye el WebView entero: de fondo queda solo este proceso
//! y mpv, que es lo que lo hace liviano.

mod discord;
mod library;
mod media_keys;
mod mpv;
mod player;
mod tools;
mod updater;
mod youtube;

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, RwLock};

use haze_core::mqtt::{self, ALL_SONGS};
use haze_core::settings::Settings;
use haze_core::song::unique_files;
use serde::Deserialize;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::window::{Effect, EffectsBuilder};
use tauri::{AppHandle, Emitter, Manager, RunEvent, WebviewUrl, WebviewWindowBuilder, WindowEvent};
use tokio::sync::watch;

use library::{Library, Song};
use player::{Player, PlayerState, SharedPlayer};

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
    pub update: Mutex<updater::Pending>,
    pub discord: discord::Discord,
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
    /// Sube o baja el volumen en esa cantidad (atajos de teclado).
    VolumeBy(f64),
    Shuffle(bool),
    Repeat(String),
    PlayFolder(String),
    /// Un tema al azar de toda la biblioteca.
    PlayRandom,
    /// Salta a la entrada de la cola con ese uid (desde HA).
    JumpUid(u64),
}

fn songs_sorted(mut songs: Vec<Arc<Song>>) -> Vec<Arc<Song>> {
    songs.sort_by_cached_key(|s| s.title.to_lowercase());
    songs
}

pub fn control(app: &AppHandle, c: Control) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let state = app.state::<AppState>();
        let folder_songs = match &c {
            Control::PlayFolder(path) => {
                let lib = state.library.read().unwrap();
                let songs: Vec<_> = if path == ALL_SONGS {
                    unique_files(&lib.songs)
                } else {
                    lib.songs.iter().filter(|s| &s.folder == path).cloned().collect()
                };
                Some(songs_sorted(songs))
            }
            Control::PlayRandom => Some(songs_sorted(unique_files(&state.library.read().unwrap().songs))),
            _ => None,
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
            Control::VolumeBy(d) => {
                let v = p.state().volume + d;
                p.set_volume(v);
            }
            Control::Shuffle(on) => p.set_shuffle(on),
            Control::Repeat(m) => p.set_repeat(m),
            Control::PlayFolder(path) => {
                let name = if path == ALL_SONGS { path.clone() } else { folder_name(&path) };
                p.play_context(folder_songs.unwrap_or_default(), 0, name);
            }
            Control::PlayRandom => p.play_random(folder_songs.unwrap_or_default(), ALL_SONGS.into()),
            Control::JumpUid(uid) => p.jump_uid(uid),
        }
    });
}

fn folder_name(path: &str) -> String {
    path.rsplit('/').next().unwrap_or(path).to_string()
}

/// Home Assistant le habla al reproductor de escritorio.
struct Ha(AppHandle);

impl mqtt::Hooks for Ha {
    fn folders(&self) -> Vec<(String, String)> {
        let state = self.0.state::<AppState>();
        let lib = state.library.read().unwrap();
        mqtt::folder_options(lib.songs.iter().map(|s| s.folder.as_str()))
    }

    fn command(&self, c: mqtt::Command) {
        use mqtt::Command as C;
        control(&self.0, match c {
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
        });
    }

    fn status(&self, s: &str) {
        *self.0.state::<AppState>().ha_status.lock().unwrap() = s.to_string();
        let _ = self.0.emit("ha://status", s);
    }

    fn cover(&self, song: &Song) -> Option<Vec<u8>> {
        media_keys::cover_bytes(song)
    }

    fn queue(&self) -> player::QueueView {
        self.0.state::<AppState>().player.blocking_lock().queue_view()
    }
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
    discord: discord::Status,
    downloads: Vec<youtube::DownloadJob>,
    update: Option<updater::UpdateInfo>,
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
    let update = state.update.lock().unwrap().info.clone();
    Bootstrap {
        settings,
        songs,
        state: pstate,
        position,
        engine,
        scanning,
        ha_status,
        discord: discord::status(&app),
        downloads,
        update,
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
    let (sources_changed, ha_changed, bg_changed, keys_changed, bot_changed, mute_changed) = {
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
        // Lo de reproduccion lo maneja el reproductor, no la pantalla de ajustes, y la
        // posicion de las ventanas la guardan las ventanas: la copia de la interfaz esta vieja.
        new.playback = cur.playback.clone();
        new.behavior.mini_position = cur.behavior.mini_position;
        new.behavior.main_window = cur.behavior.main_window;
        let changes = (
            new.sources != cur.sources,
            serde_json::to_string(&new.ha).ok() != serde_json::to_string(&cur.ha).ok(),
            new.appearance.background != cur.appearance.background,
            new.shortcuts != cur.shortcuts,
            (new.discord.enabled, &new.discord.token) != (cur.discord.enabled, &cur.discord.token),
            new.discord.mute_local != cur.discord.mute_local,
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
    if keys_changed {
        apply_shortcuts(&app);
    }
    if mute_changed {
        state.discord.mirror.set_mute_local(s.discord.mute_local);
        let app = app.clone();
        tauri::async_runtime::spawn(async move { discord::refresh_local_volume(&app).await });
    }
    if bot_changed {
        let app = app.clone();
        tauri::async_runtime::spawn(async move { discord::apply(&app).await });
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

/// La ventana `label` ya se ve: se cierra la otra. La grande y la mini se reemplazan,
/// nunca conviven. Se espera a que la nueva este dibujada para que no haya parpadeo, y
/// destruir la grande libera su WebView entero.
#[tauri::command]
async fn window_ready(app: AppHandle, label: String) {
    let other = if label == "mini" { "main" } else { "mini" };
    if let Some(w) = app.get_webview_window(other) {
        let _ = w.destroy();
    }
}

#[tauri::command]
async fn main_show(app: AppHandle) {
    show_main(&app);
}

#[tauri::command]
async fn quit(app: AppHandle) {
    quit_app(&app);
}

/// Estado de la actualizacion: None, bajando o lista para instalar.
#[tauri::command]
fn update_status(app: AppHandle) -> Option<updater::UpdateInfo> {
    app.state::<AppState>().update.lock().unwrap().info.clone()
}

/// Busca una version nueva en el momento y, si hay, la empieza a bajar.
#[tauri::command]
async fn check_update(app: AppHandle) -> Result<Option<String>, String> {
    updater::check_now(&app).await
}

/// "Actualizar" en la interfaz: instala ya y vuelve tal cual estaba.
#[tauri::command]
async fn update_now(app: AppHandle) {
    updater::apply(&app).await;
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

/// Para depurar sin pantalla: `HAZE_DEVTOOLS_PORT=9333` abre el protocolo de
/// DevTools del WebView en ese puerto. Hay que repetir los argumentos que Tauri pone
/// por defecto, porque este los reemplaza.
fn browser_args() -> Option<String> {
    let port = std::env::var("HAZE_DEVTOOLS_PORT").ok()?;
    Some(format!(
        "--disable-features=msWebOOUI,msPdfOOUI,msSmartScreenProtection --remote-debugging-port={port}"
    ))
}

/// Abre una ventana despues de que vuelva el handler que la pidio. Tauri corre los de la
/// bandeja y su menu con un Mutex tomado, y crear el WebView bombea mensajes: si en ese
/// rato llega otro evento de la bandeja (con solo pasar el mouse), Tauri vuelve a tomar
/// ese Mutex en el mismo hilo y la app queda colgada para siempre. Al prender la PC el
/// WebView tarda segundos en nacer y pasaba casi seguro. `run_on_main_thread` desde el
/// hilo principal corre en el acto, por eso se encola desde otro.
fn defer(app: &AppHandle, open: fn(&AppHandle)) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let a = app.clone();
        let _ = app.run_on_main_thread(move || open(&a));
    });
}

/// Mientras nace el WebView se siguen atendiendo eventos, y otro pedido de la misma
/// ventana llegaria antes de que exista: se crearia dos veces.
static OPENING_MAIN: AtomicBool = AtomicBool::new(false);
static OPENING_MINI: AtomicBool = AtomicBool::new(false);

pub fn show_main(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
        return;
    }
    if OPENING_MAIN.swap(true, Ordering::SeqCst) {
        return;
    }
    // Vuelve a donde estaba y con el mismo tamaño. Si ese lugar ya no esta en ninguna
    // pantalla (monitor desconectado), conserva el tamaño y aparece centrada.
    let saved = app.state::<AppState>().settings.lock().unwrap().behavior.main_window;
    let saved = saved.filter(|b| b.width > 0 && b.height > 0);
    let place = saved.is_some_and(|b| on_screen(app, b.x, b.y));
    let mut b = WebviewWindowBuilder::new(app, "main", WebviewUrl::App("index.html".into()))
        .title("Haze")
        .inner_size(1120.0, 720.0)
        .min_inner_size(780.0, 520.0)
        .decorations(false)
        .transparent(true)
        .shadow(true)
        .visible(false);
    if !place {
        b = b.center();
    }
    if let Some(args) = browser_args() {
        b = b.additional_browser_args(&args);
    }
    if let Some(fx) = glass_effects(app) {
        b = b.effects(fx);
    }
    let built = b.build();
    OPENING_MAIN.store(false, Ordering::SeqCst);
    let Ok(w) = built else { return };

    // En pixeles fisicos, como se guardo. Nace oculta: no se la ve saltar. Maximizar una
    // ventana oculta solo la marca, y se maximiza al mostrarse.
    //
    // Primero la posicion y despues el tamaño: si el lugar guardado esta en otro monitor
    // con otra escala, al llegar ahi Windows la reescala, y el tamaño quedaria mal.
    if let Some(s) = saved {
        if place {
            place_outer(&w, Some((s.x, s.y)), None);
        }
        place_outer(&w, None, Some((s.width, s.height)));
        if !place {
            let _ = w.center();
        }
        if s.maximized {
            let _ = w.maximize();
        }
    }

    // Moverla o agrandarla solo actualiza la memoria; el archivo se escribe al cerrarla.
    // Windows avisa que se movio antes de avisar que se maximizo o minimizo, y esa
    // posicion no es la de la ventana normal: se guarda la anterior para deshacerlo.
    let app2 = app.clone();
    let before_move = Mutex::new(None::<(i32, i32)>);
    w.on_window_event(move |e| {
        let state = app2.state::<AppState>();
        let maximized = || app2.get_webview_window("main").and_then(|w| w.is_maximized().ok()).unwrap_or(false);
        match e {
            WindowEvent::CloseRequested { .. } => {
                let to_tray = state.settings.lock().unwrap().behavior.close_to_tray;
                if !to_tray {
                    quit_app(&app2);
                }
            }
            WindowEvent::Moved(p) => {
                let mut prev = before_move.lock().unwrap();
                if maximized() {
                    *prev = None;
                    return;
                }
                let mut s = state.settings.lock().unwrap();
                let b = s.behavior.main_window.get_or_insert_with(Default::default);
                *prev = Some((b.x, b.y));
                (b.x, b.y) = (p.x, p.y);
            }
            WindowEvent::Resized(size) => {
                let max = maximized();
                let prev = before_move.lock().unwrap().take();
                // El de afuera, como se restaura (ver `place_outer`).
                let outer = app2.get_webview_window("main").and_then(|w| w.outer_size().ok());
                let mut s = state.settings.lock().unwrap();
                let b = s.behavior.main_window.get_or_insert_with(Default::default);
                if max || size.width == 0 || size.height == 0 {
                    if let Some((x, y)) = prev {
                        (b.x, b.y) = (x, y);
                    }
                    if max {
                        b.maximized = true;
                    }
                } else if let Some(o) = outer {
                    (b.width, b.height, b.maximized) = (o.width, o.height, false);
                }
            }
            WindowEvent::Destroyed => {
                state.settings.lock().unwrap().save(&state.settings_path);
            }
            _ => {}
        }
    });
}

/// Mueve o agranda la ventana por afuera, en pixeles fisicos, en el acto.
///
/// No se usa `set_size`: ese fija el tamaño de adentro y, para saber cuanto agregar de
/// borde, tao mira la ventana tal como esta. Recien creada y oculta todavia tiene el marco
/// de Windows (barra de titulo incluida) y no el de la ventana sin bordes, asi que sumaba
/// 30 px de alto de mas: cada vuelta desde el mini la ventana volvia mas alta.
fn place_outer(w: &tauri::WebviewWindow, pos: Option<(i32, i32)>, size: Option<(u32, u32)>) {
    #[cfg(windows)]
    {
        use windows::Win32::Foundation::HWND;
        use windows::Win32::UI::WindowsAndMessaging::{SetWindowPos, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER};
        let Ok(hwnd) = w.hwnd() else { return };
        let (x, y) = pos.unwrap_or_default();
        let (cx, cy) = size.unwrap_or_default();
        let mut flags = SWP_NOZORDER | SWP_NOACTIVATE;
        if pos.is_none() {
            flags |= SWP_NOMOVE;
        }
        if size.is_none() {
            flags |= SWP_NOSIZE;
        }
        unsafe {
            let _ = SetWindowPos(HWND(hwnd.0), None, x, y, cx as i32, cy as i32, flags);
        }
    }
    #[cfg(not(windows))]
    {
        if let Some((x, y)) = pos {
            let _ = w.set_position(tauri::PhysicalPosition::new(x, y));
        }
        if let Some((width, height)) = size {
            let _ = w.set_size(tauri::PhysicalSize::new(width, height));
        }
    }
}

/// El atajo del mini va rotando: mini -> grande -> de fondo (sin ventanas, sigue sonando)
/// -> mini. Si la grande estaba minimizada u oculta, pasa al mini.
fn cycle_windows(app: &AppHandle) {
    if app.get_webview_window("mini").is_some() {
        show_main(app);
        return;
    }
    let main = app.get_webview_window("main");
    let showing = main.as_ref().is_some_and(|w| {
        w.is_visible().unwrap_or(false) && !w.is_minimized().unwrap_or(false)
    });
    match main {
        // Destruirla (no `close`) no pasa por "cerrar a la bandeja": queda de fondo aunque
        // ese ajuste este apagado, y libera el WebView.
        Some(w) if showing => {
            let _ = w.destroy();
        }
        _ => open_mini(app),
    }
}

pub fn open_mini(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("mini") {
        let _ = w.show();
        let _ = w.set_focus();
        return;
    }
    if OPENING_MINI.swap(true, Ordering::SeqCst) {
        return;
    }
    let (on_top, pos) = {
        let s = app.state::<AppState>().settings.lock().unwrap().behavior.clone();
        (s.mini_on_top, s.mini_position)
    };
    let mut b = WebviewWindowBuilder::new(app, "mini", WebviewUrl::App("index.html#mini".into()))
        .title("Haze - mini")
        .inner_size(240.0, 84.0)
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
    let built = b.build();
    OPENING_MINI.store(false, Ordering::SeqCst);
    let Ok(w) = built else { return };

    // Vuelve a donde estaba. La primera vez, o si ese lugar ya no esta en ninguna
    // pantalla (monitor desconectado), va abajo a la derecha, arriba de la barra de
    // tareas. La ventana nace oculta, asi que no se la ve saltar.
    let target = pos.filter(|&(x, y)| on_screen(app, x, y)).or_else(|| bottom_right(&w));
    if let Some((x, y)) = target {
        let _ = w.set_position(tauri::PhysicalPosition::new(x, y));
    }

    // Se guarda en pixeles fisicos, igual que se restaura: con la pantalla al 125 % o
    // 150 % mezclar fisicos con logicos la abria corrida. Moverla solo actualiza la
    // memoria; el archivo se escribe al cerrarla.
    let app2 = app.clone();
    w.on_window_event(move |e| {
        let state = app2.state::<AppState>();
        match e {
            WindowEvent::Moved(p) => {
                state.settings.lock().unwrap().behavior.mini_position = Some((p.x, p.y));
            }
            WindowEvent::Destroyed => {
                let s = state.settings.lock().unwrap();
                s.save(&state.settings_path);
            }
            _ => {}
        }
    });
}

/// Esquina de abajo a la derecha del area de trabajo del monitor principal (la pantalla
/// sin la barra de tareas, este donde este), con un margen.
fn bottom_right(w: &tauri::WebviewWindow) -> Option<(i32, i32)> {
    let m = w.primary_monitor().ok()??;
    let area = m.work_area();
    let size = w.outer_size().ok()?;
    let margin = (12.0 * m.scale_factor()) as i32;
    Some((
        area.position.x + area.size.width as i32 - size.width as i32 - margin,
        area.position.y + area.size.height as i32 - size.height as i32 - margin,
    ))
}

/// Si el punto (esquina de la mini, mas un margen para que se pueda agarrar) cae en
/// algun monitor conectado.
fn on_screen(app: &AppHandle, x: i32, y: i32) -> bool {
    let (px, py) = (x + 40, y + 20);
    app.available_monitors().unwrap_or_default().iter().any(|m| {
        let (p, sz) = (m.position(), m.size());
        px >= p.x && py >= p.y && px < p.x + sz.width as i32 && py < p.y + sz.height as i32
    })
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
        .tooltip("Haze")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, e| match e.id.as_ref() {
            "toggle" => control(app, Control::Toggle),
            "next" => control(app, Control::Next),
            "prev" => control(app, Control::Previous),
            "mini" => defer(app, open_mini),
            "open" => defer(app, show_main),
            "quit" => quit_app(app),
            _ => {}
        })
        .on_tray_icon_event(|tray, e| {
            if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = e {
                defer(tray.app_handle(), show_main);
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
                None => "Haze".into(),
            };
            // Windows corta el tooltip en 127 caracteres.
            let tip: String = tip.chars().take(120).collect();
            let _ = tray.set_tooltip(Some(&tip));
            let _ = toggle.set_text(if s.playing { "Pausar" } else { "Reproducir" });
        }
    });
    Ok(())
}

// --------------------------------------------------------------------- atajos globales

/// Atajos que andan desde cualquier programa, con la app en segundo plano o con la
/// ventana cerrada. Las teclas salen de los ajustes (por defecto, Ctrl+Alt+Shift+...).
fn register_shortcuts(app: &AppHandle) -> tauri::Result<()> {
    use tauri_plugin_global_shortcut::{Shortcut, ShortcutState};

    app.plugin(
        tauri_plugin_global_shortcut::Builder::new()
            .with_handler(|app, shortcut, event| {
                if event.state() != ShortcutState::Pressed {
                    return;
                }
                let shortcuts = app.state::<AppState>().settings.lock().unwrap().shortcuts.clone();
                let action = {
                    shortcuts
                        .list()
                        .into_iter()
                        .find(|(_, acc)| acc.parse::<Shortcut>().is_ok_and(|sc| sc.id() == shortcut.id()))
                        .map(|(name, _)| name)
                };
                match action {
                    Some("toggle") => control(app, Control::Toggle),
                    Some("next") => control(app, Control::Next),
                    Some("previous") => control(app, Control::Previous),
                    Some("volumeUp") => control(app, Control::VolumeBy(5.0)),
                    Some("volumeDown") => control(app, Control::VolumeBy(-5.0)),
                    Some("random") => control(app, Control::PlayRandom),
                    Some("mini") => cycle_windows(app),
                    _ => {}
                }
            })
            .build(),
    )?;
    apply_shortcuts(app);
    Ok(())
}

/// Registra los atajos de los ajustes, reemplazando los que hubiera.
fn apply_shortcuts(app: &AppHandle) {
    use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut};

    let gs = app.global_shortcut();
    let _ = gs.unregister_all();
    let shortcuts = app.state::<AppState>().settings.lock().unwrap().shortcuts.clone();
    let mut taken = Vec::new();
    for (_, acc) in shortcuts.list() {
        let Ok(sc) = acc.parse::<Shortcut>() else { continue };
        // Si otro programa ya lo tiene tomado, ese atajo no anda pero los demas si.
        if let Err(e) = gs.register(sc) {
            log::warn!("atajo {acc} ocupado: {e}");
            taken.push(acc.to_string());
        }
    }
    if !taken.is_empty() {
        let _ = app.emit("shortcuts://taken", taken);
    }
}

/// Mientras se elige un atajo nuevo en Ajustes se sueltan todos: si no, Windows se
/// come la combinacion y la ventana nunca la ve.
#[tauri::command]
fn shortcuts_pause(app: AppHandle, paused: bool) {
    use tauri_plugin_global_shortcut::GlobalShortcutExt;
    if paused {
        let _ = app.global_shortcut().unregister_all();
    } else {
        apply_shortcuts(&app);
    }
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
    let device = mqtt::Device { model: "Haze (escritorio)", version: app.package_info().version.to_string() };
    state.mqtt.lock().unwrap().restart(
        tauri::async_runtime::handle().inner(),
        cfg,
        device,
        state.player_rx.clone(),
        Arc::new(Ha(app.clone())),
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

/// Sale de la app. `app.exit()` quedaba trabado cerrando los WebViews (llegaba el
/// ExitRequested pero nunca el Exit) y el proceso seguia vivo. Como `shutdown` ya
/// guardo todo, se sale directo; mpv muere con nosotros por el Job de Windows.
pub fn quit_app(app: &AppHandle) {
    shutdown(app);
    app.cleanup_before_exit();
    std::process::exit(0);
}

pub(crate) fn shutdown(app: &AppHandle) {
    discord::shutdown(app);
    let state = app.state::<AppState>();
    if let Ok(p) = state.player.try_lock() {
        p.save_session();
    }
    // Al salir con una ventana abierta, su posicion todavia esta solo en memoria.
    state.settings.lock().unwrap().save(&state.settings_path);
    state.mqtt.lock().unwrap().stop();
}

async fn start_engine(app: AppHandle, resume_playing: bool) {
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
            // El bot de Discord repite lo que hace mpv; sin canal no hace nada.
            p.attach(discord::Tee { local: m, mirror: state.discord.mirror.clone() });
            if resume || resume_playing {
                let lib = state.library.read().unwrap();
                p.restore(|id| lib.get(id));
            }
            // Tras una actualizacion, la musica sigue donde estaba.
            if resume_playing {
                p.set_paused(false);
            }
        }
        Err(e) => {
            let _ = app.emit("app://error", e);
            return;
        }
    }
    mpv::pump(state.player.clone(), rx);
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
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| defer(app, show_main)))
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
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
            let discord_cfg = settings.discord.clone();
            let cache_path = data_dir.join("library.json");
            let mut library = Library::default();
            library.set(Library::load_cache(&cache_path));

            let (player, player_rx) = Player::new(
                player::Windows(handle.clone()),
                settings.playback.volume,
                settings.playback.shuffle,
                settings.playback.repeat.clone(),
                data_dir.join("session.json"),
            );
            // Si venimos de una actualizacion, se vuelve a como estaba la app; si no, se
            // respeta "iniciar escondida" y el --hidden del arranque con Windows.
            let relaunch = updater::take_relaunch(&handle);
            let start_hidden = match &relaunch {
                Some(r) => !r.show_main,
                None => settings.behavior.start_hidden || std::env::args().any(|a| a == "--hidden"),
            };

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
                update: Mutex::new(Default::default()),
                discord: discord::Discord::new(discord_cfg.mute_local),
            });
            if discord_cfg.enabled {
                let h = handle.clone();
                tauri::async_runtime::spawn(async move { discord::apply(&h).await });
            }

            build_tray(&handle)?;
            register_shortcuts(&handle)?;
            let h = handle.clone();
            media_keys::start(app.state::<AppState>().player_rx.clone(), move |c| control(&h, c));
            persist_playback(&handle);
            start_mqtt(&handle);
            start_scan(&handle);
            let resume_playing = relaunch.as_ref().is_some_and(|r| r.playing);
            tauri::async_runtime::spawn(start_engine(handle.clone(), resume_playing));
            updater::start(handle.clone());

            if !start_hidden {
                show_main(&handle);
            } else if relaunch.as_ref().is_some_and(|r| r.show_mini) {
                open_mini(&handle);
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
            update_status,
            check_update,
            shortcuts_pause,
            update_now,
            window_ready,
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
