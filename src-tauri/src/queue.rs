//! Cola de reproduccion: port de `PlayQueue.kt` de la app de Android, con la misma
//! semantica.
//!
//! El "contexto" es la lista desde la que arrancaste (una carpeta, todas las canciones).
//! Encima podes encolar temas sueltos (efimeros): suenan a continuacion y despues se
//! vuelve solo al contexto, sin haberlo modificado.
//!
//! Todo es inmutable: cada operacion devuelve una cola nueva.

use std::sync::Arc;

use serde::Serialize;

use crate::library::Song;

#[derive(Clone, Debug, Serialize)]
pub struct QueueEntry {
    /// Identifica *esta aparicion*: la misma cancion puede estar encolada dos veces.
    pub uid: u64,
    pub song: Arc<Song>,
    /// Encolada a mano ("Reproducir a continuacion" / "Agregar a la cola"). Se consume.
    pub ephemeral: bool,
}

#[derive(Clone, Debug, Default)]
pub struct PlayQueue {
    pub entries: Vec<QueueEntry>,
    pub current_index: Option<usize>,
}

impl PlayQueue {
    pub fn current(&self) -> Option<&QueueEntry> {
        self.current_index.and_then(|i| self.entries.get(i))
    }

    fn next_slot(&self) -> usize {
        self.current_index.map_or(0, |i| i + 1)
    }

    /// Solo los temas encolados a mano que todavia no sonaron.
    pub fn pending_ephemeral(&self) -> Vec<QueueEntry> {
        self.entries[self.next_slot().min(self.entries.len())..]
            .iter()
            .filter(|e| e.ephemeral)
            .cloned()
            .collect()
    }

    /// El contexto sin los temas encolados a mano.
    pub fn context_entries(&self) -> Vec<QueueEntry> {
        self.entries.iter().filter(|e| !e.ephemeral).cloned().collect()
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub fn from_context(songs: &[Arc<Song>], start: usize, uid_from: u64) -> Self {
        if songs.is_empty() {
            return Self::default();
        }
        Self {
            entries: songs
                .iter()
                .enumerate()
                .map(|(i, s)| QueueEntry { uid: uid_from + i as u64, song: s.clone(), ephemeral: false })
                .collect(),
            current_index: Some(start.min(songs.len() - 1)),
        }
    }

    /// Suena inmediatamente despues de la actual; la ultima agregada queda primera.
    pub fn play_next(&self, song: Arc<Song>, uid: u64) -> Self {
        self.insert_at(self.next_slot(), QueueEntry { uid, song, ephemeral: true })
    }

    /// Al final de la cola efimera, antes de que se retome el contexto.
    pub fn add_to_queue(&self, song: Arc<Song>, uid: u64) -> Self {
        let mut i = self.next_slot();
        while i < self.entries.len() && self.entries[i].ephemeral {
            i += 1;
        }
        self.insert_at(i, QueueEntry { uid, song, ephemeral: true })
    }

    fn insert_at(&self, index: usize, entry: QueueEntry) -> Self {
        let target = index.min(self.entries.len());
        let mut entries = self.entries.clone();
        entries.insert(target, entry);
        let current_index = match self.current_index {
            // Encolar sobre una cola vacia: esa entrada pasa a ser la actual.
            None => Some(0),
            Some(c) if target <= c => Some(c + 1),
            c => c,
        };
        Self { entries, current_index }
    }

    /// Mueve la reproduccion a `index`. Lo efimero que ya sono se consume, pero lo que
    /// salteaste no se pierde: pasa a sonar detras de la elegida, en el mismo orden.
    pub fn move_to_index(&self, index: usize) -> Self {
        if index >= self.entries.len() {
            return self.clone();
        }
        let cur = self.current_index;
        let target = self.entries[index].clone();
        let skipped: Vec<_> = self
            .entries
            .iter()
            .enumerate()
            .filter(|(i, e)| e.ephemeral && cur.map_or(true, |c| *i > c) && *i < index)
            .map(|(_, e)| e.clone())
            .collect();
        let before: Vec<_> = self.entries[..index].iter().filter(|e| !e.ephemeral).cloned().collect();
        let new_index = before.len();
        let mut entries = before;
        entries.push(target);
        entries.extend(skipped);
        entries.extend(self.entries[index + 1..].iter().cloned());
        Self { entries, current_index: Some(new_index) }
    }

    pub fn remove_at(&self, index: usize) -> Self {
        if index >= self.entries.len() {
            return self.clone();
        }
        let mut entries = self.entries.clone();
        entries.remove(index);
        let current_index = match self.current_index {
            _ if entries.is_empty() => None,
            Some(c) if index < c => Some(c - 1),
            // Se borro la que sonaba: queda apuntando a la que ocupa su lugar.
            Some(c) if index == c => Some(c.min(entries.len() - 1)),
            c => c,
        };
        Self { entries, current_index }
    }

    /// Reordena arrastrando una entrada de `from` a `to`.
    pub fn move_entry(&self, from: usize, to: usize) -> Self {
        let n = self.entries.len();
        if from >= n || to >= n || from == to {
            return self.clone();
        }
        let current_uid = self.current().map(|e| e.uid);
        let mut entries = self.entries.clone();
        let moved = entries.remove(from);
        entries.insert(to, moved);
        let current_index = current_uid.and_then(|u| entries.iter().position(|e| e.uid == u));
        Self { entries, current_index }
    }

    /// Reordena el contexto (aleatorio) sin cortar lo que suena y dejando los
    /// encolados a mano pegados detras de la actual.
    pub fn with_context_order(&self, order: Vec<QueueEntry>) -> Self {
        let Some(cur) = self.current().cloned() else {
            let current_index = if order.is_empty() { None } else { Some(0) };
            return Self { entries: order, current_index };
        };
        let ephemeral = self.pending_ephemeral();
        let pos = order.iter().position(|e| e.uid == cur.uid);
        let entries: Vec<_> = match pos {
            Some(p) if !cur.ephemeral => order[..=p]
                .iter()
                .cloned()
                .chain(ephemeral)
                .chain(order[p + 1..].iter().cloned())
                .collect(),
            _ => std::iter::once(cur.clone())
                .chain(ephemeral)
                .chain(order.into_iter().filter(|e| e.uid != cur.uid))
                .collect(),
        };
        let current_index = entries.iter().position(|e| e.uid == cur.uid);
        Self { entries, current_index }
    }

    /// Cambia el contexto conservando lo encolado a mano, que se reinserta justo despues
    /// de la que arranca.
    pub fn replacing_context(&self, order: Vec<QueueEntry>, start: usize) -> Self {
        if order.is_empty() {
            return Self::default();
        }
        let from = start.min(order.len() - 1);
        let mut entries = order;
        let pending = self.pending_ephemeral();
        entries.splice(from + 1..from + 1, pending);
        Self { entries, current_index: Some(from) }
    }

    /// Vacia la cola efimera pendiente sin tocar el contexto ni lo que suena.
    pub fn clear_ephemeral(&self) -> Self {
        let current_uid = self.current().map(|e| e.uid);
        let entries: Vec<_> = self
            .entries
            .iter()
            .enumerate()
            .filter(|(i, e)| self.current_index.map_or(false, |c| *i <= c) || !e.ephemeral)
            .map(|(_, e)| e.clone())
            .collect();
        let current_index = current_uid.and_then(|u| entries.iter().position(|e| e.uid == u));
        Self { entries, current_index }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::library::{Song, SongKind};

    fn song(n: u64) -> Arc<Song> {
        Arc::new(Song {
            id: n,
            path: format!("C:/m/{n}.mp3"),
            title: format!("t{n}"),
            artist: String::new(),
            album: String::new(),
            duration_ms: 0,
            mtime: 0,
            folder: "C:/m".into(),
            kind: SongKind::Local,
            thumb: None,
            has_cover: false,
        })
    }

    fn ids(q: &PlayQueue) -> Vec<u64> {
        q.entries.iter().map(|e| e.song.id).collect()
    }

    fn ctx(n: u64, start: usize) -> PlayQueue {
        let songs: Vec<_> = (1..=n).map(song).collect();
        PlayQueue::from_context(&songs, start, 0)
    }

    #[test]
    fn from_context_starts_at_index() {
        let q = ctx(3, 1);
        assert_eq!(q.current().unwrap().song.id, 2);
        assert!(PlayQueue::from_context(&[], 0, 0).current().is_none());
    }

    #[test]
    fn play_next_last_added_goes_first() {
        let q = ctx(3, 0).play_next(song(10), 100).play_next(song(11), 101);
        assert_eq!(ids(&q), vec![1, 11, 10, 2, 3]);
    }

    #[test]
    fn add_to_queue_goes_after_ephemeral_block() {
        let q = ctx(3, 0).add_to_queue(song(10), 100).add_to_queue(song(11), 101);
        assert_eq!(ids(&q), vec![1, 10, 11, 2, 3]);
    }

    #[test]
    fn enqueue_on_empty_becomes_current() {
        let q = PlayQueue::default().add_to_queue(song(5), 1);
        assert_eq!(q.current().unwrap().song.id, 5);
    }

    #[test]
    fn move_to_index_keeps_skipped_ephemeral() {
        // actual 1, encoladas 10 11 12, contexto 2 3. Tocar la 12 no pierde 10 y 11.
        let q = ctx(3, 0)
            .add_to_queue(song(10), 100)
            .add_to_queue(song(11), 101)
            .add_to_queue(song(12), 102)
            .move_to_index(3);
        assert_eq!(ids(&q), vec![1, 12, 10, 11, 2, 3]);
        assert_eq!(q.current().unwrap().song.id, 12);
    }

    #[test]
    fn move_to_index_consumes_played_ephemeral() {
        let q = ctx(3, 0).add_to_queue(song(10), 100).move_to_index(1).move_to_index(2);
        assert_eq!(ids(&q), vec![1, 2, 3]);
        assert_eq!(q.current().unwrap().song.id, 2);
    }

    #[test]
    fn remove_current_points_to_next() {
        let q = ctx(3, 1).remove_at(1);
        assert_eq!(q.current().unwrap().song.id, 3);
        let q = ctx(3, 2).remove_at(2);
        assert_eq!(q.current().unwrap().song.id, 2);
        let q = ctx(1, 0).remove_at(0);
        assert!(q.current().is_none());
    }

    #[test]
    fn move_entry_tracks_current() {
        let q = ctx(4, 1).move_entry(1, 3);
        assert_eq!(ids(&q), vec![1, 3, 4, 2]);
        assert_eq!(q.current().unwrap().song.id, 2);
    }

    #[test]
    fn replacing_context_keeps_pending_ephemeral() {
        let q = ctx(3, 0).add_to_queue(song(10), 100);
        let other: Vec<_> = (20..23).map(|n| QueueEntry { uid: 200 + n, song: song(n), ephemeral: false }).collect();
        let q = q.replacing_context(other, 1);
        assert_eq!(ids(&q), vec![20, 21, 10, 22]);
        assert_eq!(q.current().unwrap().song.id, 21);
    }

    #[test]
    fn with_context_order_keeps_current_and_ephemeral() {
        let q = ctx(4, 1).add_to_queue(song(10), 100);
        let mut order = q.context_entries();
        order.reverse(); // 4 3 2 1
        let q = q.with_context_order(order);
        assert_eq!(ids(&q), vec![4, 3, 2, 10, 1]);
        assert_eq!(q.current().unwrap().song.id, 2);
    }

    #[test]
    fn clear_ephemeral_leaves_context() {
        let q = ctx(3, 0).add_to_queue(song(10), 100).clear_ephemeral();
        assert_eq!(ids(&q), vec![1, 2, 3]);
        assert_eq!(q.current().unwrap().song.id, 1);
    }
}
