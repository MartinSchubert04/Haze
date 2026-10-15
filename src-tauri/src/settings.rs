//! Preferencias del usuario. Se guardan como JSON en la carpeta de datos de la app
//! (`%APPDATA%\com.mrtn.haze\settings.json`).
//!
//! Todos los campos tienen valor por defecto: un archivo viejo, al que le falta algo
//! que se agrego despues, se sigue leyendo sin perder lo demas.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Settings {
    /// Carpetas de donde sale la biblioteca.
    pub sources: Vec<String>,
    /// Carpetas favoritas para guardar lo que se baja de YouTube.
    pub download_folders: Vec<String>,
    /// La que se propone primero al bajar.
    pub default_download_folder: Option<String>,
    pub appearance: Appearance,
    pub playback: Playback,
    pub behavior: Behavior,
    pub ha: HomeAssistant,
    pub song_sort: String,
    pub folder_sort: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Appearance {
    pub accent: String,
    /// "glass" (escritorio desenfocado), "kiosk" (fondo del kiosco), "image" o "color".
    pub background: String,
    pub background_color: String,
    pub background_image: Option<String>,
    /// 0..1: cuanto se oscurece el fondo para que el texto se lea.
    pub dim: f32,
    /// Desenfoque extra de la imagen de fondo, en px.
    pub blur: f32,
    /// Teñir la pantalla de reproduccion con el color de la caratula.
    pub tint_from_artwork: bool,
    pub folder_color: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Playback {
    pub volume: f64,
    pub shuffle: bool,
    /// "off", "all" o "one".
    pub repeat: String,
    /// Retomar la ultima cancion al abrir.
    pub resume: bool,
    pub last_song: Option<String>,
    pub last_context: Option<String>,
    pub last_position: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Behavior {
    /// Cerrar la ventana la manda a la bandeja en vez de salir.
    pub close_to_tray: bool,
    pub start_hidden: bool,
    pub mini_on_top: bool,
    pub mini_position: Option<(i32, i32)>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct HomeAssistant {
    pub enabled: bool,
    pub host: String,
    pub port: u16,
    pub username: String,
    pub password: String,
    /// Nombre del dispositivo en HA. De ahi salen los entity_id.
    pub device_name: String,
}

impl Default for Settings {
    fn default() -> Self {
        let music = dirs_music();
        Self {
            sources: music.iter().cloned().collect(),
            download_folders: music.iter().cloned().collect(),
            default_download_folder: music,
            appearance: Appearance::default(),
            playback: Playback::default(),
            behavior: Behavior::default(),
            ha: HomeAssistant::default(),
            song_sort: "title_asc".into(),
            folder_sort: "name_asc".into(),
        }
    }
}

impl Default for Appearance {
    fn default() -> Self {
        Self {
            accent: "#f9e2af".into(),
            background: "glass".into(),
            background_color: "#0f1720".into(),
            background_image: None,
            dim: 0.45,
            blur: 0.0,
            tint_from_artwork: true,
            folder_color: "#f9e2af".into(),
        }
    }
}

impl Default for Playback {
    fn default() -> Self {
        Self {
            volume: 70.0,
            shuffle: false,
            repeat: "all".into(),
            resume: true,
            last_song: None,
            last_context: None,
            last_position: 0.0,
        }
    }
}

impl Default for Behavior {
    fn default() -> Self {
        Self { close_to_tray: true, start_hidden: false, mini_on_top: true, mini_position: None }
    }
}

impl Default for HomeAssistant {
    fn default() -> Self {
        Self {
            enabled: false,
            host: String::new(),
            port: 1883,
            username: String::new(),
            password: String::new(),
            device_name: "Haze PC".into(),
        }
    }
}

fn dirs_music() -> Option<String> {
    std::env::var("USERPROFILE")
        .ok()
        .map(|home| PathBuf::from(home).join("Music"))
        .filter(|p| p.is_dir())
        .map(|p| p.to_string_lossy().replace('\\', "/"))
}

impl Settings {
    pub fn load(path: &PathBuf) -> Self {
        std::fs::read_to_string(path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    /// Escribe a un temporal y renombra: si se corta la luz a mitad, no queda un JSON roto.
    pub fn save(&self, path: &PathBuf) {
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let tmp = path.with_extension("json.tmp");
        if let Ok(json) = serde_json::to_string_pretty(self) {
            if std::fs::write(&tmp, json).is_ok() {
                let _ = std::fs::rename(&tmp, path);
            }
        }
    }
}
