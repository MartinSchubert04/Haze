//! Actualizaciones automaticas, lo mas invisibles posible:
//!
//! 1. Al minuto de arrancar y cada 6 horas se fija en GitHub si hay una version nueva.
//! 2. Si hay, la baja de fondo (el instalador pesa ~3 MB) y verifica la firma.
//! 3. Se instala sola en cuanto la app esta ociosa: sin ventanas abiertas y sin musica
//!    sonando. El instalador corre en silencio y vuelve a abrir la app, que queda en la
//!    bandeja igual que antes.
//! 4. Si estas escuchando, no corta: la interfaz muestra "Actualizar" y, si lo tocas, la
//!    app vuelve tal cual estaba (ventana abierta y la cancion sonando en el mismo segundo).

use std::path::PathBuf;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_updater::{Update, UpdaterExt};

use crate::AppState;

const FIRST_CHECK: Duration = Duration::from_secs(60);
const CHECK_EVERY: Duration = Duration::from_secs(6 * 3600);
const IDLE_POLL: Duration = Duration::from_secs(60);

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInfo {
    pub version: String,
    /// "downloading" o "ready".
    pub status: String,
}

#[derive(Default)]
pub struct Pending {
    pub info: Option<UpdateInfo>,
    ready: Option<(Update, Vec<u8>)>,
}

/// Como estaba la app antes de actualizarse, para dejarla igual al volver.
#[derive(Serialize, Deserialize, Default)]
pub struct Relaunch {
    pub show_main: bool,
    pub show_mini: bool,
    pub playing: bool,
}

fn relaunch_path(app: &AppHandle) -> Option<PathBuf> {
    app.path().app_local_data_dir().ok().map(|d| d.join("relaunch.json"))
}

/// Lo que dejo la version anterior antes de actualizarse (se lee una sola vez).
pub fn take_relaunch(app: &AppHandle) -> Option<Relaunch> {
    let path = relaunch_path(app)?;
    let r = std::fs::read(&path).ok().and_then(|b| serde_json::from_slice(&b).ok());
    let _ = std::fs::remove_file(&path);
    r
}

fn set_info(app: &AppHandle, info: Option<UpdateInfo>) {
    app.state::<AppState>().update.lock().unwrap().info = info.clone();
    let _ = app.emit("update://status", info);
}

pub fn start(app: AppHandle) {
    // Busca y baja.
    let a = app.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(FIRST_CHECK).await;
        loop {
            if let Err(e) = check_and_download(&a).await {
                log::warn!("no se pudo buscar actualizaciones: {e}");
            }
            tokio::time::sleep(CHECK_EVERY).await;
        }
    });
    // Instala cuando nadie la esta usando.
    tauri::async_runtime::spawn(async move {
        loop {
            tokio::time::sleep(IDLE_POLL).await;
            let ready = app.state::<AppState>().update.lock().unwrap().ready.is_some();
            if ready && is_idle(&app).await {
                apply(&app).await;
            }
        }
    });
}

/// "Buscar actualizaciones" en Ajustes: lo mismo que el chequeo periodico, pero ya.
/// Devuelve la version nueva (que queda bajandose o lista) o None si esta al dia.
pub async fn check_now(app: &AppHandle) -> Result<Option<String>, String> {
    check_and_download(app).await?;
    Ok(app.state::<AppState>().update.lock().unwrap().info.as_ref().map(|i| i.version.clone()))
}

async fn check_and_download(app: &AppHandle) -> Result<(), String> {
    if app.state::<AppState>().update.lock().unwrap().info.is_some() {
        return Ok(());
    }
    let updater = app.updater().map_err(|e| e.to_string())?;
    let Some(update) = updater.check().await.map_err(|e| e.to_string())? else {
        return Ok(());
    };
    let version = update.version.clone();
    set_info(app, Some(UpdateInfo { version: version.clone(), status: "downloading".into() }));
    match update.download(|_, _| {}, || {}).await {
        Ok(bytes) => {
            app.state::<AppState>().update.lock().unwrap().ready = Some((update, bytes));
            set_info(app, Some(UpdateInfo { version, status: "ready".into() }));
            Ok(())
        }
        Err(e) => {
            // Se reintenta en el proximo chequeo.
            set_info(app, None);
            Err(e.to_string())
        }
    }
}

async fn is_idle(app: &AppHandle) -> bool {
    let no_windows = app.get_webview_window("main").is_none() && app.get_webview_window("mini").is_none();
    // Instalar reinicia la app: cortaria lo que se esta bajando de fondo.
    no_windows && !crate::youtube::busy(app) && !app.state::<AppState>().player.lock().await.state().playing
}

/// Instala lo que se bajo. No vuelve: el instalador cierra la app y la vuelve a abrir.
pub async fn apply(app: &AppHandle) {
    let state = app.state::<AppState>();
    let Some((update, bytes)) = state.update.lock().unwrap().ready.take() else { return };
    let playing = state.player.lock().await.state().playing;
    let relaunch = Relaunch {
        show_main: app.get_webview_window("main").is_some(),
        show_mini: app.get_webview_window("mini").is_some(),
        playing,
    };
    if let (Some(path), Ok(json)) = (relaunch_path(app), serde_json::to_vec(&relaunch)) {
        let _ = std::fs::write(path, json);
    }
    crate::shutdown(app);
    spawn_relaunch_watchdog();
    if let Err(e) = update.install(&bytes) {
        // No se pudo lanzar el instalador: la app sigue como estaba.
        if let Some(path) = relaunch_path(app) {
            let _ = std::fs::remove_file(path);
        }
        set_info(app, None);
        let _ = app.emit("app://error", format!("No se pudo actualizar: {e}"));
    }
}

/// Respaldo por si el instalador no vuelve a abrir la app (lo hace "como el usuario
/// del escritorio" y eso puede fallar, por ejemplo en una sesion remota desconectada).
/// Un PowerShell oculto espera a que se reemplace el ejecutable y, si Haze no arranco
/// sola, la abre con los mismos argumentos. No queda nada corriendo despues.
fn spawn_relaunch_watchdog() {
    let Ok(exe) = std::env::current_exe() else { return };
    let args: Vec<String> = std::env::args().skip(1).map(|a| format!("'{}'", a.replace('\'', "''"))).collect();
    let exe_ps = exe.to_string_lossy().replace('\'', "''");
    let script = format!(
        "$exe = '{exe_ps}'; $antes = (Get-Item $exe).LastWriteTime; \
         for ($i = 0; $i -lt 120; $i++) {{ \
           Start-Sleep 1; \
           if ((Get-Item $exe -ErrorAction SilentlyContinue).LastWriteTime -ne $antes) {{ break }} \
         }}; \
         Start-Sleep 6; \
         if (-not (Get-Process | Where-Object {{ $_.Path -eq $exe }})) {{ Start-Process $exe {} }}",
        if args.is_empty() { String::new() } else { format!("-ArgumentList {}", args.join(",")) }
    );
    let mut cmd = std::process::Command::new("powershell");
    cmd.args(["-NoProfile", "-NonInteractive", "-Command", &script]);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // Solo CREATE_NO_WINDOW: con DETACHED_PROCESS PowerShell arranca sin consola y
        // se cierra sin hacer nada. Igual sobrevive a que la app se cierre.
        cmd.creation_flags(0x0800_0000);
    }
    let _ = cmd.spawn();
}
