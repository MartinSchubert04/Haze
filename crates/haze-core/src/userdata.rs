//! Lo que es de cada usuario y no de la carpeta de musica: las listas y las canciones dadas
//! de baja. Se guarda en el server (`DATA_DIR/users/<user>.json`) y cada aparato tiene una
//! copia para usarla fuera de casa; al sincronizar se juntan (ver [`UserData::merge`]).
//!
//! Las canciones se nombran por su clave de archivo ([`crate::song::file_key`]: nombre y
//! tamaño), que es la misma en la PC, el telefono y el server aunque cambie la carpeta.
//! Dar de baja no borra el archivo: solo lo oculta en todos los aparatos.

use std::collections::{HashMap, HashSet};
use std::path::Path;

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct UserData {
    pub lists: Vec<List>,
    pub removed: Vec<Removed>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct List {
    pub id: String,
    pub name: String,
    /// Claves de archivo, en el orden de la lista.
    pub songs: Vec<u64>,
    /// Ultimo cambio, en ms epoch: al juntar dos copias gana la mas nueva.
    pub updated: i64,
    /// Borrada: queda la marca para que otra copia vieja no la reviva.
    pub deleted: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Removed {
    pub file: u64,
    /// Para mostrarla en "Dadas de baja" aunque este aparato no tenga el archivo.
    pub title: String,
    /// false: se restauro.
    pub removed: bool,
    pub updated: i64,
}

/// Un cambio pedido desde la pantalla.
#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum Op {
    #[serde(rename_all = "camelCase")]
    Create {
        #[serde(default)]
        id: String,
        name: String,
        #[serde(default)]
        files: Vec<u64>,
    },
    Rename { id: String, name: String },
    Delete { id: String },
    Add { id: String, files: Vec<u64> },
    Remove { id: String, file: u64 },
    /// Dar de baja (o restaurar) canciones: `(clave, titulo)`.
    SetRemoved { files: Vec<(u64, String)>, removed: bool },
}

/// Gana el cambio mas nuevo; si empatan, el borrado (para que no reviva).
fn newer<T>(a: &T, b: &T, key: impl Fn(&T) -> (i64, bool)) -> bool {
    key(a) > key(b)
}

impl UserData {
    pub fn load(path: &Path) -> Self {
        std::fs::read(path)
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default()
    }

    /// Escribe a un temporal y renombra, como los ajustes.
    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let tmp = path.with_extension("json.tmp");
        std::fs::write(&tmp, serde_json::to_vec_pretty(self).unwrap_or_default())?;
        std::fs::rename(&tmp, path)
    }

    /// Junta dos copias: por lista y por cancion dada de baja, gana el cambio mas nuevo. Da
    /// lo mismo el orden (`a.merge(b) == b.merge(a)`), asi el server y el aparato llegan a
    /// lo mismo.
    pub fn merge(&self, other: &UserData) -> UserData {
        let mut lists: HashMap<&str, &List> = HashMap::new();
        for l in self.lists.iter().chain(&other.lists) {
            match lists.get(l.id.as_str()) {
                Some(cur) if !newer(l, cur, |x| (x.updated, x.deleted)) && !tie_break(l, cur) => {}
                _ => {
                    lists.insert(&l.id, l);
                }
            }
        }
        let mut removed: HashMap<u64, &Removed> = HashMap::new();
        for r in self.removed.iter().chain(&other.removed) {
            match removed.get(&r.file) {
                Some(cur) if !newer(r, cur, |x| (x.updated, x.removed)) && !(r.updated == cur.updated && r.removed == cur.removed && r.title > cur.title) => {}
                _ => {
                    removed.insert(r.file, r);
                }
            }
        }
        let mut lists: Vec<List> = lists.into_values().cloned().collect();
        lists.sort_by(|a, b| a.id.cmp(&b.id));
        let mut removed: Vec<Removed> = removed.into_values().cloned().collect();
        removed.sort_by_key(|r| r.file);
        UserData { lists, removed }
    }

    /// Las claves dadas de baja.
    pub fn removed_set(&self) -> HashSet<u64> {
        self.removed.iter().filter(|r| r.removed).map(|r| r.file).collect()
    }

    /// Las listas vivas, por nombre.
    pub fn visible_lists(&self) -> Vec<&List> {
        let mut out: Vec<&List> = self.lists.iter().filter(|l| !l.deleted).collect();
        out.sort_by_cached_key(|l| l.name.to_lowercase());
        out
    }

    /// Aplica un cambio con la hora `now` (ms). Devuelve el id de la lista tocada.
    pub fn apply(&mut self, op: Op, now: i64) -> Option<String> {
        match op {
            Op::Create { id, name, files } => {
                let id = if id.is_empty() { new_id(now) } else { id };
                let name = clean_name(&name)?;
                let mut songs = Vec::new();
                push_unique(&mut songs, &files);
                self.lists.retain(|l| l.id != id);
                self.lists.push(List { id: id.clone(), name, songs, updated: now, deleted: false });
                Some(id)
            }
            Op::Rename { id, name } => {
                let name = clean_name(&name)?;
                let l = self.list_mut(&id)?;
                l.name = name;
                l.updated = now;
                Some(id)
            }
            Op::Delete { id } => {
                let l = self.list_mut(&id)?;
                l.deleted = true;
                l.songs.clear();
                l.updated = now;
                Some(id)
            }
            Op::Add { id, files } => {
                let l = self.list_mut(&id)?;
                push_unique(&mut l.songs, &files);
                l.updated = now;
                Some(id)
            }
            Op::Remove { id, file } => {
                let l = self.list_mut(&id)?;
                l.songs.retain(|f| *f != file);
                l.updated = now;
                Some(id)
            }
            Op::SetRemoved { files, removed } => {
                for (file, title) in files {
                    match self.removed.iter_mut().find(|r| r.file == file) {
                        Some(r) => {
                            r.removed = removed;
                            r.updated = now;
                            if !title.is_empty() {
                                r.title = title;
                            }
                        }
                        None => self.removed.push(Removed { file, title, removed, updated: now }),
                    }
                }
                None
            }
        }
    }

    fn list_mut(&mut self, id: &str) -> Option<&mut List> {
        self.lists.iter_mut().find(|l| l.id == id && !l.deleted)
    }
}

/// Mismo momento en dos aparatos: se elige por contenido para que el resultado no dependa
/// del orden.
fn tie_break(a: &List, b: &List) -> bool {
    a.updated == b.updated && a.deleted == b.deleted && (a.name.as_str(), &a.songs) > (b.name.as_str(), &b.songs)
}

fn clean_name(name: &str) -> Option<String> {
    let n: String = name.trim().chars().take(80).collect();
    (!n.is_empty()).then_some(n)
}

fn push_unique(songs: &mut Vec<u64>, files: &[u64]) {
    for f in files {
        if *f != 0 && !songs.contains(f) {
            songs.push(*f);
        }
    }
}

/// Unico entre aparatos: la hora y algo del proceso y del reloj fino.
fn new_id(now: i64) -> String {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.subsec_nanos())
        .unwrap_or(0);
    let salt = crate::song::id_for(&format!("{now}|{nanos}|{}", std::process::id())) & 0xffff_ffff;
    format!("{now:x}-{salt:08x}")
}

/// Ahora, en ms epoch.
pub fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn data_with(ops: Vec<(Op, i64)>) -> UserData {
        let mut d = UserData::default();
        for (op, t) in ops {
            d.apply(op, t);
        }
        d
    }

    #[test]
    fn create_add_remove() {
        let mut d = UserData::default();
        let id = d.apply(Op::Create { id: String::new(), name: " Rock ".into(), files: vec![1, 2, 2] }, 10).unwrap();
        assert_eq!(d.lists[0].name, "Rock");
        assert_eq!(d.lists[0].songs, [1, 2]);
        d.apply(Op::Add { id: id.clone(), files: vec![2, 3] }, 11);
        d.apply(Op::Remove { id: id.clone(), file: 1 }, 12);
        assert_eq!(d.lists[0].songs, [2, 3]);
        assert_eq!(d.lists[0].updated, 12);
        assert!(d.apply(Op::Create { id: String::new(), name: "  ".into(), files: vec![] }, 13).is_none());
    }

    #[test]
    fn newest_change_wins_and_order_does_not_matter() {
        let base = data_with(vec![(Op::Create { id: "a".into(), name: "Rock".into(), files: vec![1] }, 10)]);
        let mut phone = base.clone();
        phone.apply(Op::Add { id: "a".into(), files: vec![2] }, 20);
        let mut pc = base.clone();
        pc.apply(Op::Rename { id: "a".into(), name: "Rock viejo".into() }, 15);
        pc.apply(Op::Create { id: "b".into(), name: "Pop".into(), files: vec![] }, 16);

        let m1 = phone.merge(&pc);
        let m2 = pc.merge(&phone);
        assert_eq!(m1, m2);
        let a = m1.lists.iter().find(|l| l.id == "a").unwrap();
        assert_eq!(a.songs, [1, 2]);
        assert_eq!(m1.lists.len(), 2);
    }

    #[test]
    fn deleted_lists_do_not_come_back() {
        let base = data_with(vec![(Op::Create { id: "a".into(), name: "Rock".into(), files: vec![1] }, 10)]);
        let mut pc = base.clone();
        pc.apply(Op::Delete { id: "a".into() }, 20);
        let m = base.merge(&pc);
        assert!(m.visible_lists().is_empty());
        // Una edicion posterior en otro lado no la revive: la lista borrada no se edita.
        let mut phone = m.clone();
        assert!(phone.apply(Op::Add { id: "a".into(), files: vec![3] }, 30).is_none());
    }

    #[test]
    fn removed_and_restored() {
        let mut d = UserData::default();
        d.apply(Op::SetRemoved { files: vec![(7, "Tema".into())], removed: true }, 10);
        assert!(d.removed_set().contains(&7));
        let mut other = d.clone();
        other.apply(Op::SetRemoved { files: vec![(7, String::new())], removed: false }, 20);
        let m = d.merge(&other);
        assert!(m.removed_set().is_empty());
        assert_eq!(m.removed[0].title, "Tema");
    }
}
