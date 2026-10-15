//! El reproductor: une la cola con mpv y publica el estado.
//!
//! El estado "grande" (que suena, pausa, volumen, aleatorio...) sale por un canal
//! `watch`: lo escuchan la bandeja, el panel de medios de Windows y Home Assistant.
//! La posicion va aparte y solo a las ventanas, porque cambia varias veces por segundo.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tauri::{AppHandle, Emitter};
use tokio::sync::{mpsc, watch, Mutex};

use crate::library::{Song, SongKind};
use crate::mpv::{Mpv, MpvEvent};
use crate::queue::{PlayQueue, QueueEntry};

#[derive(Clone, Debug, Default, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PlayerState {
    pub song: Option<Arc<Song>>,
    pub playing: bool,
    /// Cargando un stream o esperando el buffer.
    pub buffering: bool,
    pub duration: f64,
    pub volume: f64,
    pub shuffle: bool,
    pub repeat: String,
    /// De donde viene lo que suena: "Todas las canciones", el nombre de una carpeta...
    pub context: String,
    /// Sube cada vez que cambia la cola, para que la pantalla de cola se refresque.
    pub queue_version: u64,
    /// Posicion al momento del ultimo cambio de estado (HA la extrapola).
    pub position: f64,
    pub error: Option<String>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QueueView {
    pub entries: Vec<QueueEntry>,
    pub current_index: Option<usize>,
    pub context: String,
}

/// Lo que se guarda para retomar al abrir la app.
#[derive(Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Session {
    pub context: String,
    pub entries: Vec<SessionEntry>,
    pub current: Option<usize>,
    pub position: f64,
}

#[derive(Serialize, Deserialize)]
pub struct SessionEntry {
    pub id: u64,
    pub ephemeral: bool,
    /// Los streams no estan en la biblioteca: se guardan enteros.
    pub stream: Option<Song>,
}

pub struct Player {
    app: AppHandle,
    mpv: Option<Mpv>,
    queue: PlayQueue,
    /// El contexto en su orden original, para volver de aleatorio.
    original: Vec<QueueEntry>,
    next_uid: u64,
    state: PlayerState,
    position: f64,
    tx: watch::Sender<PlayerState>,
    last_pos_emit: Instant,
    errors_in_a_row: u32,
    rng: u64,
    session_path: PathBuf,
    /// Se cargo un tema pero no tiene que arrancar solo (al retomar la sesion).
    start_paused: bool,
    /// mpv logro abrir la salida de audio. Solo se sabe cuando algo suena.
    has_audio_out: bool,
    /// Segundos seguidos "sonando" sin salida de audio.
    silent_ticks: u32,
}

pub type SharedPlayer = Arc<Mutex<Player>>;

impl Player {
    pub fn new(app: AppHandle, volume: f64, shuffle: bool, repeat: String, session_path: PathBuf) -> (Self, watch::Receiver<PlayerState>) {
        let state = PlayerState { volume, shuffle, repeat, ..Default::default() };
        let (tx, rx) = watch::channel(state.clone());
        let seed = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0x9e3779b97f4a7c15)
            | 1;
        (
            Self {
                app,
                mpv: None,
                queue: PlayQueue::default(),
                original: Vec::new(),
                next_uid: 1,
                state,
                position: 0.0,
                tx,
                last_pos_emit: Instant::now(),
                errors_in_a_row: 0,
                rng: seed,
                session_path,
                start_paused: false,
                has_audio_out: false,
                silent_ticks: 0,
            },
            rx,
        )
    }

    pub fn attach(&mut self, mpv: Mpv) {
        self.mpv = Some(mpv);
    }

    pub fn state(&self) -> PlayerState {
        let mut s = self.state.clone();
        s.position = self.position;
        s
    }

    pub fn has_engine(&self) -> bool {
        self.mpv.is_some()
    }

    pub fn position(&self) -> f64 {
        self.position
    }

    fn publish(&mut self) {
        self.state.position = self.position;
        let s = self.state.clone();
        let _ = self.app.emit("player://state", &s);
        self.tx.send_if_modified(|old| {
            if *old != s {
                *old = s;
                true
            } else {
                false
            }
        });
    }

    fn queue_changed(&mut self) {
        self.state.queue_version += 1;
        self.publish();
    }

    fn uid(&mut self) -> u64 {
        self.next_uid += 1;
        self.next_uid
    }

    fn random(&mut self) -> u64 {
        // xorshift64: alcanza para mezclar una lista.
        self.rng ^= self.rng << 13;
        self.rng ^= self.rng >> 7;
        self.rng ^= self.rng << 17;
        self.rng
    }

    fn shuffled(&mut self, mut v: Vec<QueueEntry>) -> Vec<QueueEntry> {
        for i in (1..v.len()).rev() {
            let j = (self.random() % (i as u64 + 1)) as usize;
            v.swap(i, j);
        }
        v
    }

    /// Carga en mpv la entrada actual de la cola.
    fn load_current(&mut self, start: f64) {
        let Some(entry) = self.queue.current().cloned() else {
            self.state.song = None;
            self.state.playing = false;
            if let Some(m) = &self.mpv {
                m.stop();
            }
            self.queue_changed();
            return;
        };
        self.position = start;
        self.state.duration = entry.song.duration_ms as f64 / 1000.0;
        self.state.song = Some(entry.song.clone());
        self.state.error = None;
        self.state.buffering = entry.song.kind == SongKind::Stream;
        if let Some(m) = &self.mpv {
            m.set("pause", json!(self.start_paused));
            m.load(&entry.song.path, start);
        }
        self.state.playing = !self.start_paused;
        self.start_paused = false;
        self.queue_changed();
        self.save_session();
    }

    // ---------------------------------------------------------------- comandos

    /// Arranca un contexto nuevo (una carpeta, la lista completa, resultados de YouTube).
    pub fn play_context(&mut self, songs: Vec<Arc<Song>>, start: usize, context: String) {
        if songs.is_empty() {
            return;
        }
        let base = self.next_uid + 1;
        self.next_uid += songs.len() as u64 + 1;
        let entries: Vec<QueueEntry> = songs
            .into_iter()
            .enumerate()
            .map(|(i, song)| QueueEntry { uid: base + i as u64, song, ephemeral: false })
            .collect();
        let start = start.min(entries.len() - 1);
        self.original = entries.clone();
        let (order, start) = if self.state.shuffle {
            // La elegida arranca primero y el resto se mezcla detras.
            let chosen = entries[start].clone();
            let rest: Vec<_> = entries.into_iter().filter(|e| e.uid != chosen.uid).collect();
            let mut order = vec![chosen];
            order.extend(self.shuffled(rest));
            (order, 0)
        } else {
            (entries, start)
        };
        self.queue = self.queue.replacing_context(order, start);
        self.state.context = context;
        self.errors_in_a_row = 0;
        self.load_current(0.0);
    }

    pub fn toggle(&mut self) {
        if self.state.song.is_none() {
            return;
        }
        self.set_paused(self.state.playing);
    }

    pub fn set_paused(&mut self, paused: bool) {
        if let Some(m) = &self.mpv {
            // Si mpv quedo parado (fin de la lista), "play" vuelve a cargar el tema.
            if !paused && !self.state.playing && self.position == 0.0 {
                if let Some(e) = self.queue.current().cloned() {
                    m.set("pause", json!(false));
                    m.load(&e.song.path, 0.0);
                }
            } else {
                m.set("pause", json!(paused));
            }
        }
        self.state.playing = !paused;
        self.publish();
        if paused {
            self.save_session();
        }
    }

    pub fn next(&mut self) {
        self.advance(true);
    }

    /// Pasados 3 segundos, "anterior" vuelve al principio del tema.
    pub fn previous(&mut self) {
        if self.position > 3.0 {
            self.seek(0.0);
            return;
        }
        let Some(cur) = self.queue.current_index else { return };
        if cur == 0 {
            self.seek(0.0);
            return;
        }
        self.queue = self.queue.move_to_index(cur - 1);
        self.load_current(0.0);
    }

    fn advance(&mut self, manual: bool) {
        let Some(cur) = self.queue.current_index else { return };
        if !manual && self.state.repeat == "one" {
            self.load_current(0.0);
            return;
        }
        if cur + 1 < self.queue.entries.len() {
            self.queue = self.queue.move_to_index(cur + 1);
            self.load_current(0.0);
        } else if self.state.repeat != "off" || manual {
            self.queue = self.queue.move_to_index(0);
            if !manual && self.state.shuffle {
                // Otra vuelta mezclada, como en el celu.
                let ctx = self.queue.context_entries();
                let order = self.shuffled(ctx);
                self.queue = self.queue.with_context_order(order);
                self.queue = self.queue.move_to_index(0);
            }
            self.load_current(0.0);
        } else {
            // Fin de la lista: queda la ultima cargada, parada al principio.
            self.state.playing = false;
            self.position = 0.0;
            self.publish();
        }
    }

    pub fn seek(&mut self, seconds: f64) {
        if let Some(m) = &self.mpv {
            m.command(json!(["seek", seconds.max(0.0), "absolute"]));
        }
        self.position = seconds.max(0.0);
        self.emit_position(true);
        self.publish();
    }

    pub fn set_volume(&mut self, v: f64) {
        let v = v.clamp(0.0, 100.0);
        if let Some(m) = &self.mpv {
            m.set("volume", json!(v));
        }
        self.state.volume = v;
        self.publish();
    }

    pub fn set_shuffle(&mut self, on: bool) {
        if on == self.state.shuffle {
            return;
        }
        self.state.shuffle = on;
        let order = if on {
            let ctx = self.queue.context_entries();
            self.shuffled(ctx)
        } else {
            let alive: std::collections::HashSet<u64> = self.queue.entries.iter().map(|e| e.uid).collect();
            self.original.iter().filter(|e| alive.contains(&e.uid)).cloned().collect()
        };
        self.queue = self.queue.with_context_order(order);
        self.queue_changed();
    }

    pub fn set_repeat(&mut self, mode: String) {
        self.state.repeat = match mode.as_str() {
            "off" | "one" => mode,
            _ => "all".into(),
        };
        self.publish();
    }

    pub fn play_next(&mut self, song: Arc<Song>) {
        let was_empty = self.queue.current().is_none();
        let uid = self.uid();
        self.queue = self.queue.play_next(song, uid);
        if was_empty {
            self.load_current(0.0);
        } else {
            self.queue_changed();
        }
    }

    pub fn add_to_queue(&mut self, song: Arc<Song>) {
        let was_empty = self.queue.current().is_none();
        let uid = self.uid();
        self.queue = self.queue.add_to_queue(song, uid);
        if was_empty {
            self.load_current(0.0);
        } else {
            self.queue_changed();
        }
    }

    pub fn jump(&mut self, index: usize) {
        self.queue = self.queue.move_to_index(index);
        self.load_current(0.0);
    }

    pub fn remove(&mut self, index: usize) {
        let removing_current = self.queue.current_index == Some(index);
        self.queue = self.queue.remove_at(index);
        if removing_current {
            self.load_current(0.0);
        } else {
            self.queue_changed();
        }
    }

    pub fn move_entry(&mut self, from: usize, to: usize) {
        self.queue = self.queue.move_entry(from, to);
        self.queue_changed();
    }

    pub fn clear_queue(&mut self) {
        self.queue = self.queue.clear_ephemeral();
        self.queue_changed();
    }

    pub fn queue_view(&self) -> QueueView {
        QueueView {
            entries: self.queue.entries.clone(),
            current_index: self.queue.current_index,
            context: self.state.context.clone(),
        }
    }

    // ---------------------------------------------------------------- eventos de mpv

    fn emit_position(&mut self, force: bool) {
        if force || self.last_pos_emit.elapsed() >= Duration::from_millis(400) {
            self.last_pos_emit = Instant::now();
            let _ = self.app.emit("player://position", json!({ "position": self.position, "duration": self.state.duration }));
        }
    }

    pub fn on_event(&mut self, ev: MpvEvent) {
        match ev {
            MpvEvent::Property(name, v) => self.on_property(&name, v),
            MpvEvent::FileLoaded => {
                self.errors_in_a_row = 0;
                self.state.buffering = false;
                self.publish();
            }
            MpvEvent::EndFile { reason, error } => match reason.as_str() {
                "eof" => self.advance(false),
                "error" => {
                    let title = self.state.song.as_ref().map(|s| s.title.clone()).unwrap_or_default();
                    self.state.error = Some(format!("No se pudo reproducir \"{title}\": {}", error.unwrap_or_default()));
                    self.state.buffering = false;
                    self.errors_in_a_row += 1;
                    if self.errors_in_a_row < 3 {
                        self.advance(false);
                    } else {
                        self.state.playing = false;
                        self.publish();
                    }
                }
                _ => {}
            },
            MpvEvent::Exited => {
                self.mpv = None;
                self.state.playing = false;
                self.state.error = Some("El motor de audio se cerro. Reabri la app.".into());
                self.publish();
            }
        }
    }

    fn on_property(&mut self, name: &str, v: Value) {
        match name {
            "time-pos" => {
                if let Some(p) = v.as_f64() {
                    self.position = p;
                    self.emit_position(false);
                }
            }
            "duration" => {
                if let Some(d) = v.as_f64() {
                    if (d - self.state.duration).abs() > 0.5 {
                        self.state.duration = d;
                        self.publish();
                    }
                }
            }
            "pause" => {
                if let Some(p) = v.as_bool() {
                    // Solo importa si hay algo cargado; mpv parado reporta pause=false.
                    if self.state.song.is_some() && self.state.playing == p {
                        self.state.playing = !p;
                        self.publish();
                    }
                }
            }
            "paused-for-cache" => {
                let b = v.as_bool().unwrap_or(false);
                if b != self.state.buffering {
                    self.state.buffering = b;
                    self.publish();
                }
            }
            "current-ao" => self.has_audio_out = v.as_str().is_some_and(|ao| ao != "null"),
            "media-title" => {
                // Los streams traen el titulo real recien al cargar.
                if let (Some(t), Some(song)) = (v.as_str(), self.state.song.clone()) {
                    if song.kind == SongKind::Stream && song.title.is_empty() && !t.is_empty() {
                        let mut s = (*song).clone();
                        s.title = t.to_string();
                        self.state.song = Some(Arc::new(s));
                        self.publish();
                    }
                }
            }
            _ => {}
        }
    }

    /// Cada segundo: si hace rato que deberia sonar y mpv no tiene salida de audio
    /// (no hay parlantes, o se desconectaron los auriculares), se avisa una vez.
    pub fn check_audio(&mut self) {
        let silent = self.state.playing && !self.state.buffering && self.state.song.is_some() && !self.has_audio_out;
        self.silent_ticks = if silent { self.silent_ticks + 1 } else { 0 };
        if self.silent_ticks == 3 {
            self.state.error = Some("No hay salida de audio: revisá que haya parlantes o auriculares conectados.".into());
            self.publish();
        }
    }

    // ---------------------------------------------------------------- sesion

    pub fn save_session(&self) {
        let session = Session {
            context: self.state.context.clone(),
            entries: self
                .queue
                .entries
                .iter()
                .map(|e| SessionEntry {
                    id: e.song.id,
                    ephemeral: e.ephemeral,
                    stream: (e.song.kind == SongKind::Stream).then(|| (*e.song).clone()),
                })
                .collect(),
            current: self.queue.current_index,
            position: self.position,
        };
        if let Ok(json) = serde_json::to_vec(&session) {
            let _ = std::fs::write(&self.session_path, json);
        }
    }

    /// Retoma la sesion anterior, en pausa.
    pub fn restore(&mut self, lookup: impl Fn(u64) -> Option<Arc<Song>>) {
        let Some(session) = std::fs::read(&self.session_path)
            .ok()
            .and_then(|b| serde_json::from_slice::<Session>(&b).ok())
        else {
            return;
        };
        let mut entries = Vec::new();
        let mut current = None;
        for (i, e) in session.entries.into_iter().enumerate() {
            let song = match e.stream {
                Some(s) => Some(Arc::new(s)),
                None => lookup(e.id),
            };
            // Lo que se borro del disco desde la ultima vez se saltea.
            if let Some(song) = song {
                if session.current == Some(i) {
                    current = Some(entries.len());
                }
                let uid = self.uid();
                entries.push(QueueEntry { uid, song, ephemeral: e.ephemeral });
            }
        }
        if entries.is_empty() {
            return;
        }
        self.original = entries.iter().filter(|e| !e.ephemeral).cloned().collect();
        self.queue = PlayQueue { current_index: Some(current.unwrap_or(0)), entries };
        self.state.context = session.context;
        self.start_paused = true;
        let pos = if current.is_some() { session.position } else { 0.0 };
        self.load_current(pos);
    }
}

/// Procesa los eventos de mpv en orden, del lado del reproductor.
pub fn pump(player: SharedPlayer, mut rx: mpsc::UnboundedReceiver<MpvEvent>) {
    tauri::async_runtime::spawn(async move {
        let mut tick = tokio::time::interval(Duration::from_secs(1));
        loop {
            tokio::select! {
                ev = rx.recv() => match ev {
                    Some(ev) => player.lock().await.on_event(ev),
                    None => return,
                },
                _ = tick.tick() => player.lock().await.check_audio(),
            }
        }
    });
}
