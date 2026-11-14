//! Sincronizar con el server de casa (apps/server, `/api/sync` y `/api/user`): la PC y el
//! telefono bajan lo que les falta, suben lo que le falta al server y juntan las listas.
//! Nunca se borra un archivo: dar de baja solo oculta (ver [`crate::userdata`]).
//!
//! Solo anda en la red de casa: si el server no contesta enseguida ([`PROBE`]), no se
//! intenta nada. Todo esto corre de fondo: abrir Haze nunca lo espera.
//! Es HTTP plano (sin TLS): asi compila para Android y el puerto no sale de la casa.

use std::collections::HashSet;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::song::file_key;

/// Puerto del server de Haze (el mismo de la tele).
pub const PORT: u16 = 7790;
/// Cada cuanto se sincroniza solo.
pub const EVERY: Duration = Duration::from_secs(2 * 60 * 60);
/// Cuanto se espera al server antes de dar por hecho que no se esta en casa. Antes eran
/// hasta 30 s: con algo en esa IP que aceptaba la conexion y no contestaba, la vuelta
/// quedaba buscando el server medio minuto cada vez que se abria Haze.
pub const PROBE: Duration = Duration::from_secs(3);

/// Un archivo, con la ruta relativa a la carpeta de musica y barras `/`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileEntry {
    pub path: String,
    pub size: u64,
    #[serde(default)]
    pub mtime: i64,
}

impl FileEntry {
    pub fn name(&self) -> &str {
        self.path.rsplit('/').next().unwrap_or(&self.path)
    }

    /// La misma clave que usan las listas y las bajas.
    pub fn key(&self) -> u64 {
        file_key(self.name(), self.size)
    }
}

/// Lo que se sincroniza: audio y letras (`.lrc`), fuera de carpetas ocultas
/// (`.stfolder`, `.thumbnails`...).
pub fn syncable(path: &str) -> bool {
    // \ y : en Windows saldrian de la carpeta (o serian un flujo de NTFS).
    if path.contains(['\\', ':']) || path.split('/').any(|c| c.is_empty() || c.starts_with('.')) {
        return false;
    }
    let ext = path.rsplit_once('.').map(|(_, e)| e.to_ascii_lowercase()).unwrap_or_default();
    ext == "lrc" || crate::song::AUDIO_EXTENSIONS.contains(&ext.as_str())
}

/// Que bajar y que subir. Se compara por clave de archivo, no por ruta: si el tema ya esta
/// en otra carpeta del aparato no se baja de nuevo. Las bajas no se bajan.
#[derive(Debug, Default, PartialEq)]
pub struct Plan {
    pub download: Vec<FileEntry>,
    pub upload: Vec<FileEntry>,
}

pub fn plan(remote: &[FileEntry], local: &[FileEntry], removed: &HashSet<u64>, wants: impl Fn(&str) -> bool) -> Plan {
    let remote_keys: HashSet<u64> = remote.iter().map(FileEntry::key).collect();
    let local_keys: HashSet<u64> = local.iter().map(FileEntry::key).collect();
    let remote_paths: HashSet<String> = remote.iter().map(|f| f.path.to_lowercase()).collect();
    let local_paths: HashSet<String> = local.iter().map(|f| f.path.to_lowercase()).collect();

    let download = remote
        .iter()
        .filter(|f| syncable(&f.path) && wants(&f.path))
        .filter(|f| !local_keys.contains(&f.key()) && !removed.contains(&f.key()))
        // Otro archivo con la misma ruta: no se pisa.
        .filter(|f| !local_paths.contains(&f.path.to_lowercase()))
        .cloned()
        .collect();
    let upload = local
        .iter()
        .filter(|f| syncable(&f.path))
        .filter(|f| !remote_keys.contains(&f.key()) && !remote_paths.contains(&f.path.to_lowercase()))
        .cloned()
        .collect();
    Plan { download, upload }
}

/// Donde vive la musica en este aparato.
pub trait Store {
    /// Los archivos que hay, relativos a la carpeta sincronizada.
    fn files(&self) -> Vec<FileEntry>;
    /// Si este aparato quiere ese archivo (Android, por ejemplo, no guarda `.lrc`).
    fn wants(&self, _path: &str) -> bool {
        true
    }
    fn open(&self, path: &str) -> std::io::Result<Box<dyn Read + Send>>;
    /// Guarda un archivo ya bajado entero en `tmp` en `path`.
    fn save(&self, tmp: &Path, path: &str, mtime: i64) -> std::io::Result<()>;
    /// Carpeta para lo que se esta bajando.
    fn tmp_dir(&self) -> PathBuf;
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Report {
    /// "ok", "away" (no se encontro el server), "error" o "running".
    pub status: String,
    /// Fin de la ultima vez, en ms epoch.
    pub at: i64,
    pub downloaded: usize,
    pub uploaded: usize,
    pub failed: usize,
    pub error: Option<String>,
}

/// "192.168.0.100" → "http://192.168.0.100:7790".
pub fn base_url(server: &str) -> Option<String> {
    let s = server.trim().trim_end_matches('/');
    if s.is_empty() {
        return None;
    }
    let s = if s.contains("://") { s.to_string() } else { format!("http://{s}") };
    let host = s.split("://").nth(1).unwrap_or("");
    Some(if host.contains(':') { s } else { format!("{s}:{PORT}") })
}

/// Nombre de usuario valido para el server: minusculas, numeros, `-` y `_`.
pub fn clean_user(user: &str) -> String {
    let u: String = user
        .trim()
        .to_lowercase()
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_')
        .take(32)
        .collect();
    if u.is_empty() {
        "default".into()
    } else {
        u
    }
}

pub fn encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' | b'/' => out.push(b as char),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

#[cfg(feature = "sync")]
pub use client::Client;

#[cfg(feature = "sync")]
mod client {
    use super::*;
    use crate::userdata::UserData;

    pub struct Client {
        /// Para lo corto (salud, listas, indice).
        quick: ureq::Agent,
        /// Para los archivos: pueden tardar.
        files: ureq::Agent,
        base: String,
        user: String,
    }

    type Result<T> = std::result::Result<T, String>;

    fn agent(connect: u64, total: Option<u64>) -> ureq::Agent {
        ureq::Agent::config_builder()
            .timeout_connect(Some(Duration::from_secs(connect)))
            .timeout_global(total.map(Duration::from_secs))
            // Que el server conteste algo; el cuerpo tiene su propio tope (ver `budget`).
            .timeout_recv_response(Some(Duration::from_secs(30)))
            .build()
            .into()
    }

    /// Tope para pasar un archivo entero. Sin tope, si se pierde la red a mitad de camino
    /// (salir de casa con el celu) la vuelta quedaba esperando para siempre y no se
    /// volvia a sincronizar hasta reiniciar la app. Alcanza de sobra hasta en un wifi
    /// lento: 30 s mas 64 KB/s (un mp3 de 10 MB, ~3 minutos).
    fn budget(size: u64) -> Option<Duration> {
        Some(Duration::from_secs(30 + size / (64 * 1024)))
    }

    impl Client {
        /// El server, si contesta: si no, este aparato no esta en casa. Casi siempre se usa
        /// Haze fuera de casa, asi que se pregunta una sola vez y por poco tiempo: en la
        /// red de casa contesta en milisegundos.
        pub fn connect(server: &str, user: &str) -> Option<Client> {
            let base = base_url(server)?;
            let quick = agent(3, Some(30));
            let v: serde_json::Value = quick
                .get(format!("{base}/api/health"))
                .config()
                .timeout_global(Some(PROBE))
                .build()
                .call()
                .ok()?
                .body_mut()
                .read_json()
                .ok()?;
            if v["app"] != "haze" {
                return None;
            }
            Some(Client { quick, files: agent(5, None), base, user: clean_user(user) })
        }

        /// Si el server sigue contestando (para cortar la vuelta si se fue la red).
        pub fn alive(&self) -> bool {
            self.quick
                .get(format!("{}/api/health", self.base))
                .config()
                .timeout_global(Some(Duration::from_secs(5)))
                .build()
                .call()
                .is_ok()
        }

        /// Manda las listas y bajas de aca y devuelve lo junto con lo del server.
        pub fn exchange(&self, data: &UserData) -> Result<UserData> {
            self.quick
                .post(format!("{}/api/user/{}", self.base, self.user))
                .send_json(data)
                .map_err(|e| e.to_string())?
                .body_mut()
                .read_json()
                .map_err(|e| e.to_string())
        }

        pub fn index(&self) -> Result<Vec<FileEntry>> {
            self.quick
                .get(format!("{}/api/sync/index", self.base))
                .call()
                .map_err(|e| e.to_string())?
                .body_mut()
                .with_config()
                .limit(64 * 1024 * 1024)
                .read_json()
                .map_err(|e| e.to_string())
        }

        pub fn download(&self, f: &FileEntry, store: &dyn Store) -> Result<()> {
            let dir = store.tmp_dir();
            std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
            let tmp = dir.join(format!("{}.part", f.key()));
            let res = (|| -> Result<()> {
                let resp = self
                    .files
                    .get(format!("{}/api/sync/file/{}", self.base, encode(&f.path)))
                    .config()
                    .timeout_global(budget(f.size))
                    .build()
                    .call()
                    .map_err(|e| e.to_string())?;
                let mut out = std::fs::File::create(&tmp).map_err(|e| e.to_string())?;
                let n = std::io::copy(&mut resp.into_body().into_reader(), &mut out).map_err(|e| e.to_string())?;
                drop(out);
                if n != f.size {
                    return Err(format!("llegaron {n} de {} bytes", f.size));
                }
                store.save(&tmp, &f.path, f.mtime).map_err(|e| e.to_string())
            })();
            let _ = std::fs::remove_file(&tmp);
            res
        }

        pub fn upload(&self, f: &FileEntry, store: &dyn Store) -> Result<()> {
            let reader = store.open(&f.path).map_err(|e| e.to_string())?;
            self.files
                .put(format!("{}/api/sync/file/{}?mtime={}", self.base, encode(&f.path), f.mtime))
                .header("Content-Length", f.size.to_string())
                .config()
                .timeout_global(budget(f.size))
                .build()
                .send(ureq::SendBody::from_owned_reader(reader))
                .map_err(|e| e.to_string())?;
            Ok(())
        }

        /// Pide al server que vuelva a leer la musica (lo que se acaba de subir).
        pub fn rescan(&self) {
            let _ = self.quick.post(format!("{}/api/rescan", self.base)).send_empty();
        }
    }

    /// Una vuelta entera: listas, despues archivos. `data` es la copia de este aparato:
    /// se reemplaza por lo junto. `progress(hechos, total)` va avisando.
    pub fn run(
        client: &Client,
        store: &dyn Store,
        data: &mut UserData,
        mut progress: impl FnMut(usize, usize),
    ) -> Report {
        let mut report = Report { status: "ok".into(), ..Default::default() };
        match client.exchange(data) {
            Ok(merged) => *data = data.merge(&merged),
            Err(e) => {
                report.status = "error".into();
                report.error = Some(format!("listas: {e}"));
                report.at = crate::userdata::now_ms();
                return report;
            }
        }
        let remote = match client.index() {
            Ok(r) => r,
            Err(e) => {
                report.status = "error".into();
                report.error = Some(format!("indice: {e}"));
                report.at = crate::userdata::now_ms();
                return report;
            }
        };
        let local = store.files();
        let p = plan(&remote, &local, &data.removed_set(), |path| store.wants(path));
        let total = p.download.len() + p.upload.len();
        let mut done = 0;
        progress(done, total);
        let mut away = false;
        for (f, down) in p.download.iter().map(|f| (f, true)).chain(p.upload.iter().map(|f| (f, false))) {
            let res = if down { client.download(f, store) } else { client.upload(f, store) };
            match res {
                Ok(()) if down => report.downloaded += 1,
                Ok(()) => report.uploaded += 1,
                Err(e) => {
                    // Se fue el server (o la red): se corta aca y se sigue la proxima vez,
                    // en vez de esperar archivo por archivo a que falle cada uno.
                    if !client.alive() {
                        away = true;
                        break;
                    }
                    report.failed += 1;
                    report.error = Some(format!("{}: {e}", f.path));
                }
            }
            done += 1;
            progress(done, total);
        }
        if report.uploaded > 0 && !away {
            client.rescan();
        }
        if away {
            report.status = "away".into();
            report.error = None;
        } else if report.failed > 0 {
            report.status = "error".into();
        }
        report.at = crate::userdata::now_ms();
        report
    }
}

#[cfg(feature = "sync")]
pub use client::run;

/// Una carpeta comun (escritorio): la carpeta sincronizada es `root`.
#[cfg(feature = "library")]
pub struct FolderStore {
    pub root: PathBuf,
    pub tmp: PathBuf,
}

#[cfg(feature = "library")]
impl FolderStore {
    fn full(&self, path: &str) -> std::io::Result<PathBuf> {
        if !syncable(path) {
            return Err(std::io::Error::new(std::io::ErrorKind::InvalidInput, "ruta invalida"));
        }
        Ok(self.root.join(path))
    }
}

#[cfg(feature = "library")]
impl Store for FolderStore {
    fn files(&self) -> Vec<FileEntry> {
        walkdir::WalkDir::new(&self.root)
            .into_iter()
            .filter_entry(|e| e.depth() == 0 || !e.file_name().to_string_lossy().starts_with('.'))
            .filter_map(Result::ok)
            .filter(|e| e.file_type().is_file())
            .filter_map(|e| {
                let rel = e.path().strip_prefix(&self.root).ok()?.to_string_lossy().replace('\\', "/");
                if !syncable(&rel) {
                    return None;
                }
                let meta = e.metadata().ok()?;
                let mtime = meta
                    .modified()
                    .ok()
                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                    .map(|d| d.as_secs() as i64)
                    .unwrap_or(0);
                Some(FileEntry { path: rel, size: meta.len(), mtime })
            })
            .collect()
    }

    fn open(&self, path: &str) -> std::io::Result<Box<dyn Read + Send>> {
        Ok(Box::new(std::fs::File::open(self.full(path)?)?))
    }

    fn save(&self, tmp: &Path, path: &str, mtime: i64) -> std::io::Result<()> {
        let dest = self.full(path)?;
        if dest.exists() {
            return Err(std::io::Error::new(std::io::ErrorKind::AlreadyExists, "ya existe"));
        }
        if let Some(dir) = dest.parent() {
            std::fs::create_dir_all(dir)?;
        }
        // El temporal puede estar en otro disco: si no se puede renombrar, se copia.
        if std::fs::rename(tmp, &dest).is_err() {
            std::fs::copy(tmp, &dest)?;
        }
        if mtime > 0 {
            if let Ok(f) = std::fs::File::options().write(true).open(&dest) {
                let _ = f.set_modified(std::time::UNIX_EPOCH + Duration::from_secs(mtime as u64));
            }
        }
        Ok(())
    }

    fn tmp_dir(&self) -> PathBuf {
        self.tmp.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn f(path: &str, size: u64) -> FileEntry {
        FileEntry { path: path.into(), size, mtime: 0 }
    }

    #[test]
    fn urls_and_users() {
        assert_eq!(base_url("192.168.0.100").unwrap(), "http://192.168.0.100:7790");
        assert_eq!(base_url("http://srv:8000/").unwrap(), "http://srv:8000");
        assert!(base_url(" ").is_none());
        assert_eq!(clean_user(" Martin "), "martin");
        assert_eq!(clean_user("../x"), "x");
        assert_eq!(clean_user(""), "default");
        assert_eq!(encode("All/Fito Páez - 11 Y 6.mp3"), "All/Fito%20P%C3%A1ez%20-%2011%20Y%206.mp3");
    }

    #[test]
    fn only_music_outside_hidden_folders() {
        assert!(syncable("All/a.mp3"));
        assert!(syncable("All/a.lrc"));
        assert!(!syncable("musica.m3u"));
        assert!(!syncable(".stversions/a.mp3"));
        assert!(!syncable("All/../a.mp3"));
        assert!(!syncable("/abs.mp3"));
        assert!(!syncable("All\\..\\..\\a.mp3"));
        assert!(!syncable("C:/a.mp3"));
    }

    #[test]
    fn plan_downloads_missing_uploads_new_and_never_overwrites() {
        let remote = vec![f("All/a.mp3", 10), f("All/b.mp3", 20), f("All/c.mp3", 30), f("All/gone.mp3", 40)];
        let local = vec![
            f("All/a.mp3", 10),       // igual
            f("Rock/b.mp3", 20),      // ya esta, en otra carpeta
            f("All/c.mp3", 31),       // misma ruta, otro archivo: no se toca
            f("All/nueva.mp3", 50),   // solo aca: se sube
            f("Sueltas/x.m3u", 1),    // no es musica
        ];
        let removed: HashSet<u64> = [f("All/gone.mp3", 40).key()].into();
        let p = plan(&remote, &local, &removed, |_| true);
        assert!(p.download.is_empty());
        assert_eq!(p.upload, vec![f("All/nueva.mp3", 50)]);

        let p = plan(&remote, &[], &HashSet::new(), |p| !p.ends_with(".lrc"));
        assert_eq!(p.download.len(), 4);
    }
}
