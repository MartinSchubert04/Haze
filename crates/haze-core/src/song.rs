//! Una cancion, venga de un archivo o de un stream. Cada app arma las suyas: escritorio
//! recorriendo carpetas, Android con MediaStore.

use std::collections::HashMap;
use std::sync::Arc;

use serde::{Deserialize, Serialize};

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
    /// El archivo, aunque aparezca en varias carpetas (ver [`file_key`]). 0 si no se sabe.
    #[serde(default)]
    pub file: u64,
}

impl Song {
    /// Que archivo es: el mismo tema en `All` y en `Categorias/Rock` da lo mismo. Sin
    /// clave (streams, un cache viejo), cada ruta cuenta como un archivo distinto.
    pub fn file_key(&self) -> u64 {
        if self.file != 0 {
            self.file
        } else {
            self.id
        }
    }
}

/// Clave de un archivo por nombre y tamaño en bytes. Asi coinciden los hardlinks de la
/// PC y tambien las copias que deja Syncthing en el telefono (que no conserva hardlinks).
pub fn file_key(name: &str, size: u64) -> u64 {
    id_for(&format!("{size}|{name}")).max(1)
}

/// Una entrada por archivo, en el orden en que aparecen. De las copias queda la de la
/// carpeta mas especifica ("Categorias/Rock Nacional" antes que "All"); si empatan, la
/// primera por ruta, para que no cambie de un escaneo a otro.
pub fn unique_files(songs: &[Arc<Song>]) -> Vec<Arc<Song>> {
    let depth = |s: &Song| s.folder.matches('/').count();
    let better = |a: &Song, b: &Song| depth(a) > depth(b) || (depth(a) == depth(b) && a.path < b.path);

    let mut out: Vec<Arc<Song>> = Vec::with_capacity(songs.len());
    let mut at: HashMap<u64, usize> = HashMap::with_capacity(songs.len());
    for s in songs {
        match at.get(&s.file_key()) {
            Some(&i) if better(s, &out[i]) => out[i] = s.clone(),
            Some(_) => {}
            None => {
                at.insert(s.file_key(), out.len());
                out.push(s.clone());
            }
        }
    }
    out
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

    fn local(path: &str, folder: &str, file: u64) -> Arc<Song> {
        Arc::new(Song {
            id: id_for(path),
            path: path.into(),
            title: "t".into(),
            artist: String::new(),
            album: String::new(),
            duration_ms: 0,
            mtime: 0,
            folder: folder.into(),
            kind: SongKind::Local,
            thumb: None,
            has_cover: false,
            file,
        })
    }

    #[test]
    fn one_entry_per_file_from_the_most_specific_folder() {
        let a = file_key("Tema.mp3", 1234);
        let b = file_key("Otro.mp3", 1234);
        let songs = vec![
            local("C:/m/All/Tema.mp3", "C:/m/All", a),
            local("C:/m/All/Otro.mp3", "C:/m/All", b),
            local("C:/m/Generos/Rock/Tema.mp3", "C:/m/Generos/Rock", a),
            local("C:/m/Categorias/Rock 2000s/Tema.mp3", "C:/m/Categorias/Rock 2000s", a),
        ];
        let unique = unique_files(&songs);
        let paths: Vec<_> = unique.iter().map(|s| s.path.as_str()).collect();
        assert_eq!(paths, ["C:/m/Categorias/Rock 2000s/Tema.mp3", "C:/m/All/Otro.mp3"]);
    }

    #[test]
    fn songs_without_a_key_stay_apart() {
        let songs = vec![local("C:/m/a/x.mp3", "C:/m/a", 0), local("C:/m/b/x.mp3", "C:/m/b", 0)];
        assert_eq!(unique_files(&songs).len(), 2);
        assert_eq!(file_key("Tema.MP3", 10), file_key("tema.mp3", 10));
        assert_ne!(file_key("Tema.mp3", 10), file_key("Tema.mp3", 11));
    }
}
