//! Listas, bajas y sincronizar con el server de casa (ver `haze_core::sync` y
//! `haze_core::userdata`), como en escritorio. Las listas se editan aunque no haya server:
//! quedan en `userdata.json` y se juntan con las del server la proxima vez que se lo
//! encuentre.
//!
//! En el telefono viene apagado y solo corre con la app viva (sin WorkManager): al abrir,
//! cada 2 horas y con "Sincronizar ahora". La carpeta sincronizada es siempre `Music/Haze`,
//! la misma donde caen las descargas de YouTube.

use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use haze_core::song::Song;
use haze_core::sync::{self as core, syncable, Client, FileEntry, Report, Store};
use haze_core::userdata::{now_ms, Op, UserData};
use serde_json::json;
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_haze_player::{HazePlayerExt, Track};

use crate::AppState;

/// La carpeta sincronizada, como la nombra MediaStore y en el disco.
const ROOT: &str = "Music/Haze";
const ROOT_PATH: &str = "/storage/emulated/0/Music/Haze";

pub struct Sync {
    data: Mutex<UserData>,
    data_path: PathBuf,
    tmp: PathBuf,
    report: Mutex<Report>,
    running: AtomicBool,
}

impl Sync {
    pub fn new(data_dir: &Path) -> Self {
        let data_path = data_dir.join("userdata.json");
        Self {
            data: Mutex::new(UserData::load(&data_path)),
            data_path,
            tmp: data_dir.join("sync-tmp"),
            report: Mutex::new(Report::default()),
            running: AtomicBool::new(false),
        }
    }

    pub fn data(&self) -> UserData {
        self.data.lock().unwrap().clone()
    }

    pub fn report(&self) -> Report {
        self.report.lock().unwrap().clone()
    }
}

/// Las canciones que se ven: sin las dadas de baja.
pub fn visible(app: &AppHandle, songs: Vec<Arc<Song>>) -> Vec<Arc<Song>> {
    let removed = app.state::<AppState>().sync.data.lock().unwrap().removed_set();
    if removed.is_empty() {
        return songs;
    }
    songs.into_iter().filter(|s| !removed.contains(&s.file_key())).collect()
}

/// Las canciones de una lista, en su orden, con las que hay en este telefono.
pub fn list_songs(app: &AppHandle, id: &str, songs: &[Arc<Song>]) -> Option<(String, Vec<Arc<Song>>)> {
    let state = app.state::<AppState>();
    let data = state.sync.data.lock().unwrap();
    let list = data.lists.iter().find(|l| l.id == id && !l.deleted)?;
    let removed = data.removed_set();
    let by_key: std::collections::HashMap<u64, Arc<Song>> =
        haze_core::song::unique_files(songs).into_iter().map(|s| (s.file_key(), s)).collect();
    let out = list.songs.iter().filter(|k| !removed.contains(k)).filter_map(|k| by_key.get(k).cloned()).collect();
    Some((list.name.clone(), out))
}

/// Opciones de Home Assistant para las listas: ("Lista: Rock", "list:<id>").
pub fn list_options(app: &AppHandle) -> Vec<(String, String)> {
    let data = app.state::<AppState>().sync.data();
    data.visible_lists().into_iter().map(|l| (format!("Lista: {}", l.name), format!("list:{}", l.id))).collect()
}

fn changed(app: &AppHandle) {
    let state = app.state::<AppState>();
    let data = state.sync.data();
    let _ = data.save(&state.sync.data_path);
    let _ = app.emit("userdata://changed", &data);
}

#[tauri::command]
pub fn userdata_edit(app: AppHandle, op: Op) -> Option<String> {
    let id = {
        let state = app.state::<AppState>();
        let mut data = state.sync.data.lock().unwrap();
        data.apply(op, now_ms())
    };
    changed(&app);
    // En casa, el cambio llega enseguida a los otros aparatos.
    let h = app.clone();
    tauri::async_runtime::spawn_blocking(move || exchange_only(&h));
    id
}

#[tauri::command]
pub fn sync_now(app: AppHandle) {
    start(&app);
}

/// Si se puede salir a la red local (Android 17). `None`: el plugin no contesta, que pasa
/// cuando la app se saco de recientes y solo queda el servicio de musica.
fn network(app: &AppHandle) -> Option<bool> {
    app.haze_player().local_network(false).ok()
}

/// Solo las listas: rapido, para despues de cada cambio.
fn exchange_only(app: &AppHandle) {
    let state = app.state::<AppState>();
    let cfg = state.settings.lock().unwrap().sync.clone();
    if !cfg.enabled || state.sync.running.load(Ordering::SeqCst) || network(app) != Some(true) {
        return;
    }
    let Some(client) = Client::connect(&cfg.server, &cfg.user) else { return };
    let mine = state.sync.data();
    if let Ok(merged) = client.exchange(&mine) {
        let mut data = state.sync.data.lock().unwrap();
        *data = data.merge(&merged);
        drop(data);
        changed(app);
    }
}

/// Una vuelta de fondo, si no hay otra andando.
pub fn start(app: &AppHandle) {
    let state = app.state::<AppState>();
    if state.sync.running.swap(true, Ordering::SeqCst) {
        return;
    }
    let app = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let report = run(&app);
        let state = app.state::<AppState>();
        state.sync.running.store(false, Ordering::SeqCst);
        let Some(report) = report else { return };
        *state.sync.report.lock().unwrap() = report.clone();
        let _ = app.emit("sync://status", &report);
        // Lo bajado entra por MediaStore: se vuelve a leer para que aparezca.
        if report.downloaded > 0 {
            let h = app.clone();
            tauri::async_runtime::spawn(async move { crate::load_library(&h).await });
        }
    });
}

/// `None`: no se pudo ni empezar y queda el estado anterior.
fn run(app: &AppHandle) -> Option<Report> {
    let state = app.state::<AppState>();
    let cfg = state.settings.lock().unwrap().sync.clone();
    if !cfg.enabled {
        return Some(Report { status: "off".into(), at: now_ms(), ..Default::default() });
    }
    // Sin el permiso, Android corta la conexion y solo se veria "fuera de casa".
    if !network(app)? {
        return Some(Report { status: "network".into(), at: now_ms(), ..Default::default() });
    }
    // Fuera de casa no se muestra nada: "sincronizando" recien cuando el server contesta.
    let Some(client) = Client::connect(&cfg.server, &cfg.user) else {
        return Some(Report { status: "away".into(), at: now_ms(), ..Default::default() });
    };
    let _ = app.emit("sync://status", Report { status: "running".into(), ..state.sync.report() });
    let mut data = state.sync.data();
    // Sin permiso para leer la musica la biblioteca se ve vacia y se bajaria todo de nuevo:
    // ahi van solo las listas.
    let tracks = if *state.access.lock().unwrap() {
        match app.haze_player().library() {
            Ok(t) => Some(t),
            Err(e) => {
                return Some(Report {
                    status: "error".into(),
                    at: now_ms(),
                    error: Some(format!("no se pudo leer la música: {e}")),
                    ..Default::default()
                })
            }
        }
    } else {
        None
    };
    let report = match tracks {
        Some(tracks) => {
            let store = MediaStoreDir { app: app.clone(), files: files(tracks), tmp: state.sync.tmp.clone() };
            core::run(&client, &store, &mut data, |done, total| {
                let _ = app.emit("sync://progress", json!({ "done": done, "total": total }));
            })
        }
        None => match client.exchange(&data) {
            Ok(merged) => {
                data = data.merge(&merged);
                Report { status: "ok".into(), at: now_ms(), ..Default::default() }
            }
            Err(e) => Report { status: "error".into(), at: now_ms(), error: Some(e), ..Default::default() },
        },
    };
    // Lo que se edito mientras tanto tambien cuenta.
    {
        let mut cur = state.sync.data.lock().unwrap();
        *cur = cur.merge(&data);
    }
    changed(app);
    Some(report)
}

/// Al abrir (despues de la biblioteca) y cada 2 horas, mientras viva la app.
pub fn schedule(app: &AppHandle) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(Duration::from_secs(10)).await;
        loop {
            start(&app);
            tokio::time::sleep(core::EVERY).await;
        }
    });
}

// --------------------------------------------------------------------- Music/Haze

/// Los temas de MediaStore que estan en Music/Haze, con la ruta relativa a esa carpeta.
fn files(tracks: Vec<Track>) -> Vec<FileEntry> {
    tracks
        .into_iter()
        .filter(|t| !t.name.is_empty() && t.size > 0)
        .filter_map(|t| Some(FileEntry { path: relative(&t.folder, &t.name)?, size: t.size, mtime: t.mtime }))
        .collect()
}

/// "Music/Haze/All" + "a.mp3" → "All/a.mp3". Lo de afuera de Music/Haze no cuenta.
fn relative(folder: &str, name: &str) -> Option<String> {
    let rest = if folder.eq_ignore_ascii_case(ROOT) {
        ""
    } else {
        let prefix = folder.get(..ROOT.len() + 1)?;
        if !prefix.eq_ignore_ascii_case(&format!("{ROOT}/")) {
            return None;
        }
        &folder[ROOT.len() + 1..]
    };
    let path = if rest.is_empty() { name.to_string() } else { format!("{rest}/{name}") };
    syncable(&path).then_some(path)
}

/// Music/Haze del telefono: se lista con MediaStore (lo leido antes de empezar), se lee
/// por la ruta (READ_MEDIA_AUDIO alcanza para el audio) y se escribe por MediaStore, que es
/// lo que deja Android sin pedir mas permisos.
struct MediaStoreDir {
    app: AppHandle,
    files: Vec<FileEntry>,
    tmp: PathBuf,
}

impl Store for MediaStoreDir {
    fn files(&self) -> Vec<FileEntry> {
        self.files.clone()
    }

    /// MediaStore no lista las letras (`.lrc`): bajarlas las volveria a bajar cada vez.
    fn wants(&self, path: &str) -> bool {
        !path.to_lowercase().ends_with(".lrc")
    }

    fn open(&self, path: &str) -> std::io::Result<Box<dyn Read + Send>> {
        if !syncable(path) {
            return Err(std::io::Error::new(std::io::ErrorKind::InvalidInput, "ruta invalida"));
        }
        Ok(Box::new(std::fs::File::open(format!("{ROOT_PATH}/{path}"))?))
    }

    fn save(&self, tmp: &Path, path: &str, mtime: i64) -> std::io::Result<()> {
        if !syncable(path) {
            return Err(std::io::Error::new(std::io::ErrorKind::InvalidInput, "ruta invalida"));
        }
        self.app.haze_player().sync_save(&tmp.to_string_lossy(), path, mtime).map_err(std::io::Error::other)
    }

    fn tmp_dir(&self) -> PathBuf {
        self.tmp.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_inside_music_haze() {
        assert_eq!(relative("Music/Haze", "a.mp3").as_deref(), Some("a.mp3"));
        assert_eq!(relative("Music/Haze/All", "a.mp3").as_deref(), Some("All/a.mp3"));
        assert_eq!(relative("music/haze/Rock/80s", "a.mp3").as_deref(), Some("Rock/80s/a.mp3"));
        assert_eq!(relative("Music/HazeViejo", "a.mp3"), None);
        assert_eq!(relative("Music", "a.mp3"), None);
        assert_eq!(relative("Music/Haze/.thumbnails", "a.mp3"), None);
    }
}
