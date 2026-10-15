//! Una cancion, venga de un archivo o de un stream. Cada app arma las suyas: escritorio
//! recorriendo carpetas, Android con MediaStore.

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
}
