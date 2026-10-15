//! Haze en Android: reproduce con Media3/ExoPlayer en un servicio en primer plano (sigue
//! sonando con la pantalla apagada o la app cerrada) y lee la biblioteca de MediaStore.
//!
//! La parte de Kotlin esta en `android/`. Desde Rust se usa con `HazePlayerExt`: el
//! reproductor de `haze-core` recibe `HazePlayer::engine` como motor y
//! `HazePlayer::listen` le pasa lo que pasa en Android.
//!
//! En otras plataformas el plugin no hace nada.

use haze_core::player::EngineEvent;
use haze_core::song::{id_for, Song, SongKind};
use serde::Deserialize;
use tauri::plugin::{Builder, TauriPlugin};
use tauri::Runtime;

#[cfg(target_os = "android")]
mod android;
#[cfg(target_os = "android")]
pub use android::{AndroidEngine, HazePlayer, HazePlayerExt};

/// Lo que avisa Android.
#[derive(Clone, Debug, PartialEq)]
pub enum PlayerEvent {
    Engine(EngineEvent),
    /// "Siguiente" o "anterior" desde la notificacion, la pantalla de bloqueo o los
    /// auriculares. La cola es de Rust: lo resuelve el reproductor.
    Next,
    Previous,
    /// Una linea de yt-dlp mientras baja (ver `haze_core::youtube::parse_progress`).
    Download { id: u64, line: String },
    /// Avance de la descarga de una actualizacion, de 0 a 1.
    UpdateProgress(f64),
}

/// Una version publicada en GitHub (`latest-android.json`).
#[derive(Clone, Debug, Deserialize)]
pub struct Release {
    pub version: String,
    pub url: String,
    pub sha256: String,
}

/// Una cancion de MediaStore.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Track {
    /// `content://media/external/audio/media/<id>`
    pub uri: String,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub duration_ms: u64,
    pub mtime: i64,
    /// Relativa al almacenamiento: "Music/Rock".
    pub folder: String,
}

impl Track {
    pub fn into_song(self) -> Song {
        // Sin tags (lo bajado sin ffmpeg), MediaStore usa el nombre del archivo como
        // titulo: "Artista - Tema".
        let (title, artist) = match self.title.split_once(" - ") {
            Some((a, t)) if self.artist.is_empty() && !a.trim().is_empty() && !t.trim().is_empty() => {
                (t.trim().to_string(), a.trim().to_string())
            }
            _ => (self.title, self.artist),
        };
        Song {
            id: id_for(&self.uri),
            path: self.uri,
            title,
            artist,
            album: self.album,
            duration_ms: self.duration_ms,
            mtime: self.mtime,
            folder: self.folder,
            kind: SongKind::Local,
            thumb: None,
            // MediaStore no dice si hay tapa sin abrir el archivo: se intenta y, si no
            // hay, la interfaz muestra el icono.
            has_cover: true,
        }
    }
}

/// Como llega cada evento desde Kotlin: `{"type": "position", "value": 12.5}`.
#[derive(Deserialize)]
#[serde(tag = "type", content = "value", rename_all = "camelCase")]
enum Raw {
    Position(f64),
    Duration(f64),
    Paused(bool),
    Buffering(bool),
    Loaded,
    Ended,
    Failed(String),
    Next,
    Previous,
    Download { id: u64, line: String },
    Update(f64),
}

impl From<Raw> for PlayerEvent {
    fn from(raw: Raw) -> Self {
        match raw {
            Raw::Position(p) => Self::Engine(EngineEvent::Position(p)),
            Raw::Duration(d) => Self::Engine(EngineEvent::Duration(d)),
            Raw::Paused(p) => Self::Engine(EngineEvent::Paused(p)),
            Raw::Buffering(b) => Self::Engine(EngineEvent::Buffering(b)),
            Raw::Loaded => Self::Engine(EngineEvent::Loaded),
            Raw::Ended => Self::Engine(EngineEvent::Ended),
            Raw::Failed(e) => Self::Engine(EngineEvent::Failed(e)),
            Raw::Next => Self::Next,
            Raw::Previous => Self::Previous,
            Raw::Download { id, line } => Self::Download { id, line },
            Raw::Update(p) => Self::UpdateProgress(p),
        }
    }
}

#[cfg_attr(not(target_os = "android"), allow(dead_code))]
fn parse(json: &str) -> Option<PlayerEvent> {
    match serde_json::from_str::<Raw>(json) {
        Ok(raw) => Some(raw.into()),
        Err(e) => {
            log::warn!("haze-player: evento desconocido {json}: {e}");
            None
        }
    }
}

pub fn init<R: Runtime>() -> TauriPlugin<R> {
    Builder::new("haze-player")
        .setup(|_app, _api| {
            #[cfg(target_os = "android")]
            {
                use tauri::Manager;
                let handle = _api.register_android_plugin("app.haze.player.plugin", "HazePlayerPlugin")?;
                _app.manage(HazePlayer::new(handle));
            }
            Ok(())
        })
        .build()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_kotlin_events() {
        let engine = |e| Some(PlayerEvent::Engine(e));
        assert_eq!(parse(r#"{"type":"position","value":12.5}"#), engine(EngineEvent::Position(12.5)));
        // JSONObject manda los Double redondos sin decimales.
        assert_eq!(parse(r#"{"type":"duration","value":180}"#), engine(EngineEvent::Duration(180.0)));
        assert_eq!(parse(r#"{"type":"paused","value":true}"#), engine(EngineEvent::Paused(true)));
        assert_eq!(parse(r#"{"type":"loaded"}"#), engine(EngineEvent::Loaded));
        assert_eq!(parse(r#"{"type":"ended"}"#), engine(EngineEvent::Ended));
        assert_eq!(parse(r#"{"type":"failed","value":"404"}"#), engine(EngineEvent::Failed("404".into())));
        assert_eq!(parse(r#"{"type":"next"}"#), Some(PlayerEvent::Next));
        assert_eq!(
            parse(r#"{"type":"download","value":{"id":3,"line":"DL 50%"}}"#),
            Some(PlayerEvent::Download { id: 3, line: "DL 50%".into() })
        );
        assert_eq!(parse(r#"{"type":"update","value":0.5}"#), Some(PlayerEvent::UpdateProgress(0.5)));
        assert_eq!(parse(r#"{"type":"bogus"}"#), None);
    }

    #[test]
    fn track_ids_follow_the_uri() {
        let uri = "content://media/external/audio/media/42";
        let t = Track {
            uri: uri.into(),
            title: "t".into(),
            artist: String::new(),
            album: String::new(),
            duration_ms: 1000,
            mtime: 0,
            folder: "Music".into(),
        };
        let s = t.into_song();
        assert_eq!(s.id, id_for(uri));
        assert_eq!(s.path, uri);
    }

    #[test]
    fn untagged_titles_give_the_artist() {
        let track = |title: &str, artist: &str| Track {
            uri: "content://x".into(),
            title: title.into(),
            artist: artist.into(),
            album: String::new(),
            duration_ms: 0,
            mtime: 0,
            folder: String::new(),
        };
        let s = track("jawed - Me at the zoo", "").into_song();
        assert_eq!((s.title.as_str(), s.artist.as_str()), ("Me at the zoo", "jawed"));
        // Con artista en los tags, el titulo queda como esta.
        let s = track("Tema - En vivo", "Banda").into_song();
        assert_eq!((s.title.as_str(), s.artist.as_str()), ("Tema - En vivo", "Banda"));
    }
}
