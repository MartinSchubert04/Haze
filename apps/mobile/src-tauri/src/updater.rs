//! Actualizaciones: cada seis horas se fija en GitHub si hay una version nueva para
//! Android y la baja de fondo. Se instala al salir de la app sin musica sonando (lo
//! decide el plugin) o con "Actualizar ahora" en Ajustes.

use std::time::Duration;

use haze_core::newer_version as newer;
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_haze_player::{HazePlayerExt, Release};

use crate::AppState;

const EVERY: Duration = Duration::from_secs(6 * 60 * 60);

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInfo {
    pub version: String,
    /// downloading, ready
    pub status: String,
    pub progress: f64,
}

fn set(app: &AppHandle, info: Option<UpdateInfo>) {
    *app.state::<AppState>().update.lock().unwrap() = info.clone();
    let _ = app.emit("update://status", info);
}

pub fn on_progress(app: &AppHandle, progress: f64) {
    let info = {
        let state = app.state::<AppState>();
        let mut u = state.update.lock().unwrap();
        match u.as_mut() {
            Some(i) if i.status == "downloading" => {
                i.progress = progress;
                i.clone()
            }
            _ => return,
        }
    };
    let _ = app.emit("update://status", Some(info));
}

/// Busca y, si hay algo nuevo, lo baja. Devuelve la version encontrada.
async fn check(app: &AppHandle) -> Result<Option<String>, String> {
    if app.state::<AppState>().update.lock().unwrap().is_some() {
        // Ya se esta bajando o esta lista.
        return Ok(None);
    }
    let h = app.clone();
    let latest = tauri::async_runtime::spawn_blocking(move || h.haze_player().update_latest())
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())?;
    let current = app.package_info().version.to_string();
    let Some(release) = latest.filter(|r: &Release| newer(&r.version, &current)) else {
        return Ok(None);
    };
    let version = release.version.clone();
    set(app, Some(UpdateInfo { version: version.clone(), status: "downloading".into(), progress: 0.0 }));
    let h = app.clone();
    let done = tauri::async_runtime::spawn_blocking(move || h.haze_player().update_download(&release)).await;
    match done {
        Ok(Ok(())) => {
            set(app, Some(UpdateInfo { version: version.clone(), status: "ready".into(), progress: 1.0 }));
            Ok(Some(version))
        }
        Ok(Err(e)) => {
            set(app, None);
            Err(e.to_string())
        }
        Err(e) => {
            set(app, None);
            Err(e.to_string())
        }
    }
}

/// Corre de fondo mientras viva la app.
pub async fn run(app: AppHandle) {
    // Que arranque primero lo demas.
    tokio::time::sleep(Duration::from_secs(60)).await;
    loop {
        if let Err(e) = check(&app).await {
            log::warn!("haze: no se pudo buscar actualizaciones: {e}");
        }
        tokio::time::sleep(EVERY).await;
    }
}

/// "Buscar actualizaciones" en Ajustes. Devuelve la version nueva, si hay.
#[tauri::command]
pub async fn update_check(app: AppHandle) -> Result<Option<String>, String> {
    check(&app).await
}

/// "Actualizar ahora". `false` si antes hay que dar el permiso de instalar apps.
#[tauri::command]
pub async fn update_install(app: AppHandle) -> Result<bool, String> {
    // La cancion y el segundo quedan guardados: al volver a abrir, sigue de ahi.
    app.state::<AppState>().player.lock().await.save_session();
    let h = app.clone();
    tauri::async_runtime::spawn_blocking(move || h.haze_player().update_install())
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())
}
