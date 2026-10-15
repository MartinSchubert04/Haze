//! Biblioteca: recorre las carpetas fuente, lee los tags y guarda un cache para que el
//! proximo arranque no tenga que volver a abrir cada archivo.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::UNIX_EPOCH;

use lofty::file::{AudioFile, TaggedFileExt};
use lofty::tag::Accessor;
use serde::{Deserialize, Serialize};
use walkdir::WalkDir;

pub const AUDIO_EXTENSIONS: &[&str] =
    &["mp3", "flac", "m4a", "aac", "ogg", "opus", "wav", "wma", "aiff", "aif", "ape", "wv", "alac"];

/// Nombres que se buscan en la carpeta cuando el archivo no trae la tapa adentro.
const FOLDER_COVERS: &[&str] = &["cover", "folder", "front", "albumart", "album"];

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SongKind {
    Local,
    Stream,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Song {
    /// Hash de la ruta, recortado a 53 bits para que JavaScript lo lea sin perder precision.
    pub id: u64,
    /// Ruta absoluta con barras `/`, o la URL si es un stream.
    pub path: String,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub duration_ms: u64,
    /// Fecha de modificacion en segundos epoch.
    pub mtime: i64,
    pub folder: String,
    pub kind: SongKind,
    /// Miniatura remota (streams de YouTube).
    pub thumb: Option<String>,
    /// Si hay tapa, embebida o en la carpeta.
    pub has_cover: bool,
}

pub fn id_for(path: &str) -> u64 {
    // FNV-1a: estable entre ejecuciones (el hasher de std no lo es).
    let mut h: u64 = 0xcbf29ce484222325;
    for b in path.to_lowercase().bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h & ((1u64 << 53) - 1)
}

pub fn normalize(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

pub fn is_audio(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| AUDIO_EXTENSIONS.contains(&e.to_ascii_lowercase().as_str()))
        .unwrap_or(false)
}

#[derive(Default)]
pub struct Library {
    pub songs: Vec<Arc<Song>>,
    by_id: HashMap<u64, Arc<Song>>,
}

impl Library {
    pub fn get(&self, id: u64) -> Option<Arc<Song>> {
        self.by_id.get(&id).cloned()
    }

    pub fn set(&mut self, songs: Vec<Song>) {
        self.songs = songs.into_iter().map(Arc::new).collect();
        self.by_id = self.songs.iter().map(|s| (s.id, s.clone())).collect();
    }

    /// Agrega o reemplaza un archivo suelto (lo que se acaba de bajar).
    pub fn upsert(&mut self, song: Song) -> Arc<Song> {
        let song = Arc::new(song);
        if let Some(i) = self.songs.iter().position(|s| s.id == song.id) {
            self.songs[i] = song.clone();
        } else {
            self.songs.push(song.clone());
        }
        self.by_id.insert(song.id, song.clone());
        song
    }

    pub fn load_cache(path: &Path) -> Vec<Song> {
        std::fs::read(path)
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default()
    }

    pub fn save_cache(&self, path: &Path) {
        let songs: Vec<&Song> = self.songs.iter().map(|s| s.as_ref()).collect();
        if let Ok(json) = serde_json::to_vec(&songs) {
            let tmp = path.with_extension("json.tmp");
            if std::fs::write(&tmp, json).is_ok() {
                let _ = std::fs::rename(&tmp, path);
            }
        }
    }
}

fn mtime_of(meta: &std::fs::Metadata) -> i64 {
    meta.modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// Recorre las fuentes. Lo que no cambio desde el cache no se vuelve a abrir.
pub fn scan(sources: &[String], cached: &[Song]) -> Vec<Song> {
    let by_path: HashMap<&str, &Song> = cached.iter().map(|s| (s.path.as_str(), s)).collect();

    let mut fresh = Vec::new();
    let mut to_read: Vec<(PathBuf, i64)> = Vec::new();
    let mut seen = std::collections::HashSet::new();

    for source in sources {
        for entry in WalkDir::new(source).follow_links(true).into_iter().filter_map(Result::ok) {
            let p = entry.path();
            if !entry.file_type().is_file() || !is_audio(p) {
                continue;
            }
            let norm = normalize(p);
            // Dos fuentes que se pisan (Music y Music/rock) no duplican canciones.
            if !seen.insert(norm.to_lowercase()) {
                continue;
            }
            let mtime = entry.metadata().map(|m| mtime_of(&m)).unwrap_or(0);
            match by_path.get(norm.as_str()) {
                Some(s) if s.mtime == mtime => fresh.push((*s).clone()),
                _ => to_read.push((p.to_path_buf(), mtime)),
            }
        }
    }

    // Leer tags es lo lento: se reparte entre los nucleos.
    let threads = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4).min(8);
    let chunk = to_read.len().div_ceil(threads).max(1);
    let read: Vec<Song> = std::thread::scope(|scope| {
        let handles: Vec<_> = to_read
            .chunks(chunk)
            .map(|part| scope.spawn(move || part.iter().map(|(p, m)| read_song(p, *m)).collect::<Vec<_>>()))
            .collect();
        handles.into_iter().flat_map(|h| h.join().unwrap_or_default()).collect()
    });

    fresh.extend(read);
    fresh
}

pub fn read_file(path: &Path) -> Option<Song> {
    let meta = std::fs::metadata(path).ok()?;
    Some(read_song(path, mtime_of(&meta)))
}

fn read_song(path: &Path, mtime: i64) -> Song {
    let norm = normalize(path);
    let stem = path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    let folder = path.parent().map(normalize).unwrap_or_default();

    let mut song = Song {
        id: id_for(&norm),
        path: norm,
        title: stem,
        artist: String::new(),
        album: String::new(),
        duration_ms: 0,
        mtime,
        folder,
        kind: SongKind::Local,
        thumb: None,
        has_cover: false,
    };

    if let Ok(tagged) = lofty::read_from_path(path) {
        song.duration_ms = tagged.properties().duration().as_millis() as u64;
        if let Some(tag) = tagged.primary_tag().or_else(|| tagged.first_tag()) {
            if let Some(t) = tag.title().filter(|t| !t.trim().is_empty()) {
                song.title = t.trim().to_string();
            }
            song.artist = tag.artist().map(|a| a.trim().to_string()).unwrap_or_default();
            song.album = tag.album().map(|a| a.trim().to_string()).unwrap_or_default();
            song.has_cover = !tag.pictures().is_empty();
        }
    }
    if !song.has_cover {
        song.has_cover = folder_cover(Path::new(&song.folder)).is_some();
    }
    song
}

fn folder_cover(folder: &Path) -> Option<PathBuf> {
    let entries = std::fs::read_dir(folder).ok()?;
    for e in entries.flatten() {
        let p = e.path();
        let ext = p.extension().and_then(|x| x.to_str()).map(|x| x.to_ascii_lowercase());
        if !matches!(ext.as_deref(), Some("jpg" | "jpeg" | "png" | "webp")) {
            continue;
        }
        let stem = p.file_stem().and_then(|s| s.to_str()).unwrap_or("").to_ascii_lowercase();
        if FOLDER_COVERS.iter().any(|c| stem.starts_with(c)) {
            return Some(p);
        }
    }
    None
}

/// Bytes y tipo MIME de la tapa: la embebida, o la de la carpeta.
pub fn cover(song: &Song) -> Option<(Vec<u8>, String)> {
    if song.kind != SongKind::Local {
        return None;
    }
    if let Ok(tagged) = lofty::read_from_path(&song.path) {
        for tag in tagged.tags() {
            if let Some(pic) = tag.pictures().first() {
                let mime = pic.mime_type().map(|m| m.as_str().to_string()).unwrap_or_else(|| "image/jpeg".into());
                return Some((pic.data().to_vec(), mime));
            }
        }
    }
    let p = folder_cover(Path::new(&song.folder))?;
    let mime = match p.extension().and_then(|x| x.to_str()).map(|x| x.to_ascii_lowercase()).as_deref() {
        Some("png") => "image/png",
        Some("webp") => "image/webp",
        _ => "image/jpeg",
    };
    Some((std::fs::read(&p).ok()?, mime.into()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_are_stable_and_js_safe() {
        let a = id_for("C:/Music/a.mp3");
        assert_eq!(a, id_for("c:/music/A.mp3"));
        assert!(a < (1u64 << 53));
        assert_ne!(a, id_for("C:/Music/b.mp3"));
    }

    #[test]
    fn audio_extensions() {
        assert!(is_audio(Path::new("x/Song.MP3")));
        assert!(is_audio(Path::new("x/a.flac")));
        assert!(!is_audio(Path::new("x/cover.jpg")));
    }
}
