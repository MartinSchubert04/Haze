//! Listas, bajas y sincronizar con el server de casa (ver `haze_core::sync` y
//! `haze_core::userdata`). Las listas se editan aca aunque no haya server: se guardan en
//! `userdata.json` y se juntan con las del server la proxima vez que se lo encuentre.
//!
//! Sincroniza solo al arrancar, cada 2 horas y con "Sincronizar ahora". Si el server no
//! contesta (fuera de casa), no hace nada hasta la proxima vuelta.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use haze_core::song::Song;
use haze_core::sync::{self as core, Client, FolderStore, Report};
use haze_core::userdata::{now_ms, Op, UserData};
use serde_json::json;
use tauri::{AppHandle, Emitter, Manager};

use crate::AppState;

pub struct Sync {
    data: Mutex<UserData>,
    data_path: PathBuf,
    tmp: PathBuf,
    report: Mutex<Report>,
    running: AtomicBool,
}

impl Sync {
    pub fn new(data_dir: &std::path::Path) -> Self {
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

/// Las canciones de una lista, en su orden, con las que hay en esta biblioteca.
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

/// Solo las listas: rapido, para despues de cada cambio.
fn exchange_only(app: &AppHandle) {
    let state = app.state::<AppState>();
    let cfg = state.settings.lock().unwrap().sync.clone();
    if !cfg.enabled || state.sync.running.load(Ordering::SeqCst) {
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
        *state.sync.report.lock().unwrap() = report.clone();
        let _ = app.emit("sync://status", &report);
        if report.downloaded > 0 {
            crate::start_scan(&app);
        }
    });
}

fn run(app: &AppHandle) -> Report {
    let state = app.state::<AppState>();
    let cfg = state.settings.lock().unwrap().sync.clone();
    if !cfg.enabled {
        return Report { status: "off".into(), at: now_ms(), ..Default::default() };
    }
    // Fuera de casa no se muestra nada: "sincronizando" recien cuando el server contesta.
    let Some(client) = Client::connect(&cfg.server, &cfg.user) else {
        return Report { status: "away".into(), at: now_ms(), ..Default::default() };
    };
    let _ = app.emit("sync://status", Report { status: "running".into(), ..state.sync.report() });
    let mut data = state.sync.data();
    let report = if cfg.folder.trim().is_empty() {
        // Sin carpeta: solo las listas.
        match client.exchange(&data) {
            Ok(merged) => {
                data = data.merge(&merged);
                Report { status: "ok".into(), at: now_ms(), ..Default::default() }
            }
            Err(e) => Report { status: "error".into(), at: now_ms(), error: Some(e), ..Default::default() },
        }
    } else {
        let store = FolderStore { root: PathBuf::from(&cfg.folder), tmp: state.sync.tmp.clone() };
        core::run(&client, &store, &mut data, |done, total| {
            let _ = app.emit("sync://progress", json!({ "done": done, "total": total }));
        })
    };
    // Lo que se edito mientras tanto tambien cuenta.
    {
        let mut cur = state.sync.data.lock().unwrap();
        *cur = cur.merge(&data);
    }
    changed(app);
    report
}

/// Al arrancar (con un rato para no competir con la biblioteca) y cada 2 horas.
pub fn schedule(app: &AppHandle) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(Duration::from_secs(30)).await;
        loop {
            start(&app);
            tokio::time::sleep(core::EVERY).await;
        }
    });
}
