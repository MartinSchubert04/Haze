//! Home Assistant por MQTT. Con MQTT discovery las entidades aparecen solas en HA,
//! agrupadas en un dispositivo: estado, cancion, artista, caratula, botones de
//! anterior/play/siguiente, volumen, aleatorio, repetir y "reproducir carpeta".
//!
//! Lo usan las dos apps: cada una pone en [`Hooks`] como obedecer las ordenes, de donde
//! salen las carpetas y las tapas, y donde mostrar si esta conectado.

use std::collections::HashSet;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use rumqttc::{AsyncClient, Event, LastWill, MqttOptions, Packet, QoS};
use serde_json::json;
use tokio::runtime::Handle;
use tokio::sync::watch;
use tokio::task::JoinHandle;

use crate::player::PlayerState;
use crate::settings::HomeAssistant;
use crate::song::Song;

const REPEAT: &[(&str, &str)] = &[("off", "No"), ("all", "Todo"), ("one", "Una")];

/// La opcion del selector de carpetas que reproduce toda la biblioteca.
pub const ALL_SONGS: &str = "Todas las canciones";

/// Lo que se puede pedir desde HA.
#[derive(Debug, Clone, PartialEq)]
pub enum Command {
    Toggle,
    Play,
    Pause,
    Next,
    Previous,
    Seek(f64),
    Volume(f64),
    Shuffle(bool),
    Repeat(String),
    /// La ruta de la carpeta, o [`ALL_SONGS`].
    PlayFolder(String),
}

/// Lo que pone cada app.
pub trait Hooks: Send + Sync + 'static {
    /// Carpetas para el selector: (nombre visible, ruta). Ver [`folder_options`].
    fn folders(&self) -> Vec<(String, String)>;
    fn command(&self, c: Command);
    /// "off", "connected" o "error".
    fn status(&self, s: &str);
    /// La tapa en JPEG. Puede bloquear: se llama fuera del runtime.
    fn cover(&self, song: &Song) -> Option<Vec<u8>>;
}

/// Como se presenta el dispositivo en HA.
pub struct Device {
    pub model: &'static str,
    pub version: String,
}

pub struct Bridge {
    task: Option<JoinHandle<()>>,
}

impl Default for Bridge {
    fn default() -> Self {
        Self::new()
    }
}

impl Bridge {
    pub fn new() -> Self {
        Self { task: None }
    }

    pub fn stop(&mut self) {
        if let Some(t) = self.task.take() {
            t.abort();
        }
    }

    /// (Re)conecta con la configuracion nueva.
    pub fn restart(
        &mut self,
        rt: &Handle,
        cfg: HomeAssistant,
        device: Device,
        rx: watch::Receiver<PlayerState>,
        hooks: Arc<dyn Hooks>,
    ) {
        self.stop();
        if !cfg.enabled || cfg.host.trim().is_empty() {
            hooks.status("off");
            return;
        }
        self.task = Some(rt.spawn(run(cfg, device, rx, hooks)));
    }
}

fn slug(s: &str) -> String {
    let mut out = String::new();
    for c in s.to_lowercase().chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c);
        } else if !out.ends_with('_') {
            out.push('_');
        }
    }
    out.trim_matches('_').to_string()
}

fn folder_name(path: &str) -> String {
    path.rsplit('/').next().unwrap_or(path).to_string()
}

/// Carpetas para el selector de HA a partir de las de la biblioteca (con `/`).
pub fn folder_options<'a>(folders: impl IntoIterator<Item = &'a str>) -> Vec<(String, String)> {
    let mut paths: Vec<&str> = folders.into_iter().collect();
    paths.sort();
    paths.dedup();
    let mut out = vec![(ALL_SONGS.to_string(), ALL_SONGS.to_string())];
    let mut used = HashSet::new();
    for p in paths {
        let mut name = folder_name(p);
        // Dos carpetas con el mismo nombre: se distinguen por la de arriba.
        if !used.insert(name.to_lowercase()) {
            let parts: Vec<&str> = p.rsplit('/').take(2).collect();
            name = format!("{} / {}", parts.get(1).unwrap_or(&""), parts[0]);
            used.insert(name.to_lowercase());
        }
        out.push((name, p.to_string()));
    }
    out.truncate(150);
    out
}

/// Traduce un mensaje entrante. `topic` viene sin el `haze/<nodo>/` del principio.
fn parse(topic: &str, payload: &str, folders: &[(String, String)]) -> Option<Command> {
    match topic {
        "cmd" => match payload {
            "toggle" => Some(Command::Toggle),
            "play" => Some(Command::Play),
            "pause" => Some(Command::Pause),
            "next" => Some(Command::Next),
            "previous" => Some(Command::Previous),
            _ => None,
        },
        "volume/set" => payload.parse().ok().map(Command::Volume),
        "seek/set" => payload.parse().ok().map(Command::Seek),
        "shuffle/set" => Some(Command::Shuffle(payload == "ON")),
        "repeat/set" => REPEAT.iter().find(|(_, l)| *l == payload).map(|(k, _)| Command::Repeat(k.to_string())),
        "folder/set" => folders.iter().find(|(name, _)| name == payload).map(|(_, path)| Command::PlayFolder(path.clone())),
        _ => None,
    }
}

async fn run(cfg: HomeAssistant, device: Device, mut rx: watch::Receiver<PlayerState>, hooks: Arc<dyn Hooks>) {
    let node = slug(&cfg.device_name);
    let base = format!("haze/{node}");
    let avail = format!("{base}/availability");

    // ID unico por proceso: si dos Haze usan el mismo ID, el broker echa a una cada vez
    // que entra la otra y las entidades quedan saltando entre "unavailable" y lo demas.
    let client_id = format!("haze-{node}-{}", std::process::id());
    let mut opts = MqttOptions::new(client_id, cfg.host.trim(), cfg.port);
    opts.set_keep_alive(Duration::from_secs(30));
    // Las caratulas pesan bastante mas que el limite por defecto.
    opts.set_max_packet_size(256 * 1024, 4 * 1024 * 1024);
    if !cfg.username.is_empty() {
        opts.set_credentials(cfg.username.clone(), cfg.password.clone());
    }
    opts.set_last_will(LastWill::new(&avail, "offline", QoS::AtLeastOnce, true));

    let (client, mut events) = AsyncClient::new(opts, 32);

    // Conexion y comandos entrantes.
    let c2 = client.clone();
    let base2 = base.clone();
    let (online_tx, mut online_rx) = tokio::sync::mpsc::unbounded_channel::<()>();
    let folder_list = Arc::new(Mutex::new(Vec::<(String, String)>::new()));
    let folder_list2 = folder_list.clone();
    let h2 = hooks.clone();
    let poll = tokio::spawn(async move {
        loop {
            match events.poll().await {
                Ok(Event::Incoming(Packet::ConnAck(_))) => {
                    h2.status("connected");
                    let _ = c2.subscribe(format!("{base2}/+/set"), QoS::AtLeastOnce).await;
                    let _ = c2.subscribe(format!("{base2}/cmd"), QoS::AtLeastOnce).await;
                    let _ = c2.subscribe("homeassistant/status", QoS::AtLeastOnce).await;
                    let _ = online_tx.send(());
                }
                Ok(Event::Incoming(Packet::Publish(p))) => {
                    let payload = String::from_utf8_lossy(&p.payload).trim().to_string();
                    if p.topic == "homeassistant/status" {
                        // HA se reinicio: hay que volver a anunciar las entidades.
                        if payload == "online" {
                            let _ = online_tx.send(());
                        }
                        continue;
                    }
                    let topic = p.topic.strip_prefix(&format!("{base2}/")).unwrap_or(&p.topic);
                    let c = parse(topic, &payload, &folder_list2.lock().unwrap());
                    if let Some(c) = c {
                        h2.command(c);
                    }
                }
                Ok(_) => {}
                Err(_) => {
                    h2.status("error");
                    tokio::time::sleep(Duration::from_secs(5)).await;
                }
            }
        }
    });

    let device = json!({
        "identifiers": [format!("haze_{node}")],
        "name": cfg.device_name,
        "manufacturer": "Haze",
        "model": device.model,
        "sw_version": device.version,
    });
    let state_topic = format!("{base}/state");

    let mut last_cover: Option<u64> = None;
    let mut last_folders: Vec<(String, String)> = Vec::new();

    loop {
        tokio::select! {
            r = rx.changed() => { if r.is_err() { break; } }
            Some(()) = online_rx.recv() => {
                // Anuncio (o re-anuncio) de las entidades.
                last_folders = hooks.folders();
                *folder_list.lock().unwrap() = last_folders.clone();
                announce(&client, &base, &avail, &state_topic, &device, &last_folders).await;
                let _ = client.publish(&avail, QoS::AtLeastOnce, true, "online").await;
                last_cover = None;
            }
        }
        let s = rx.borrow_and_update().clone();

        let f = hooks.folders();
        if f != last_folders {
            last_folders = f;
            *folder_list.lock().unwrap() = last_folders.clone();
            announce_folders(&client, &base, &device, &last_folders).await;
        }

        let song = s.song.as_ref();
        let payload = json!({
            "state": match (song, s.playing) { (None, _) => "idle", (Some(_), true) => "playing", _ => "paused" },
            "title": song.map(|x| x.title.clone()).unwrap_or_default(),
            "artist": song.map(|x| x.artist.clone()).unwrap_or_default(),
            "album": song.map(|x| x.album.clone()).unwrap_or_default(),
            "duration": s.duration.round(),
            "position": s.position.round(),
            "volume": s.volume.round(),
            "shuffle": if s.shuffle { "ON" } else { "OFF" },
            "repeat": REPEAT.iter().find(|(k, _)| *k == s.repeat).map(|(_, l)| *l).unwrap_or("Todo"),
            "context": s.context,
            "source": song.map(|x| if x.thumb.is_some() { "youtube" } else { "local" }).unwrap_or(""),
        });
        let _ = client.publish(&state_topic, QoS::AtLeastOnce, true, payload.to_string()).await;

        let id = song.map(|x| x.id);
        if id != last_cover {
            last_cover = id;
            let bytes = match song {
                Some(x) => {
                    let (x, h) = (x.clone(), hooks.clone());
                    tokio::task::spawn_blocking(move || h.cover(&x)).await.ok().flatten()
                }
                None => None,
            };
            let _ = client.publish(format!("{base}/cover"), QoS::AtLeastOnce, true, bytes.unwrap_or_default()).await;
        }
    }
    poll.abort();
}

async fn publish_config(client: &AsyncClient, component: &str, id: &str, mut cfg: serde_json::Value, device: &serde_json::Value, avail: &str) {
    cfg["unique_id"] = json!(id);
    cfg["device"] = device.clone();
    cfg["availability_topic"] = json!(avail);
    let _ = client
        .publish(format!("homeassistant/{component}/{id}/config"), QoS::AtLeastOnce, true, cfg.to_string())
        .await;
}

async fn announce(
    client: &AsyncClient,
    base: &str,
    avail: &str,
    state: &str,
    device: &serde_json::Value,
    folders: &[(String, String)],
) {
    let node = base.trim_start_matches("haze/");
    let p = |s: &str| format!("haze_{node}_{s}");
    let cmd = format!("{base}/cmd");

    publish_config(client, "sensor", &p("estado"), json!({
        "name": "Estado", "icon": "mdi:music-circle", "state_topic": state,
        "value_template": "{{ value_json.state }}", "json_attributes_topic": state,
    }), device, avail).await;
    publish_config(client, "sensor", &p("cancion"), json!({
        "name": "Cancion", "icon": "mdi:music-note", "state_topic": state,
        "value_template": "{{ value_json.title[:250] }}",
    }), device, avail).await;
    publish_config(client, "sensor", &p("artista"), json!({
        "name": "Artista", "icon": "mdi:account-music", "state_topic": state,
        "value_template": "{{ value_json.artist[:250] }}",
    }), device, avail).await;
    publish_config(client, "image", &p("caratula"), json!({
        "name": "Caratula", "image_topic": format!("{base}/cover"), "content_type": "image/jpeg",
    }), device, avail).await;
    for (key, name, icon, payload) in [
        ("anterior", "Anterior", "mdi:skip-previous", "previous"),
        ("play_pausa", "Play/Pausa", "mdi:play-pause", "toggle"),
        ("siguiente", "Siguiente", "mdi:skip-next", "next"),
    ] {
        publish_config(client, "button", &p(key), json!({
            "name": name, "icon": icon, "command_topic": cmd, "payload_press": payload,
        }), device, avail).await;
    }
    publish_config(client, "number", &p("volumen"), json!({
        "name": "Volumen", "icon": "mdi:volume-high", "min": 0, "max": 100, "step": 1, "mode": "slider",
        "unit_of_measurement": "%", "command_topic": format!("{base}/volume/set"),
        "state_topic": state, "value_template": "{{ value_json.volume }}",
    }), device, avail).await;
    publish_config(client, "switch", &p("aleatorio"), json!({
        "name": "Aleatorio", "icon": "mdi:shuffle-variant", "command_topic": format!("{base}/shuffle/set"),
        "state_topic": state, "value_template": "{{ value_json.shuffle }}",
    }), device, avail).await;
    publish_config(client, "select", &p("repetir"), json!({
        "name": "Repetir", "icon": "mdi:repeat", "options": REPEAT.iter().map(|(_, l)| *l).collect::<Vec<_>>(),
        "command_topic": format!("{base}/repeat/set"), "state_topic": state, "value_template": "{{ value_json.repeat }}",
    }), device, avail).await;
    announce_folders(client, base, device, folders).await;
}

async fn announce_folders(client: &AsyncClient, base: &str, device: &serde_json::Value, folders: &[(String, String)]) {
    let node = base.trim_start_matches("haze/");
    let avail = format!("{base}/availability");
    let mut options: Vec<&str> = folders.iter().map(|(n, _)| n.as_str()).collect();
    if options.is_empty() {
        options.push(ALL_SONGS);
    }
    publish_config(client, "select", &format!("haze_{node}_carpeta"), json!({
        "name": "Reproducir carpeta", "icon": "mdi:folder-music",
        "options": options, "command_topic": format!("{base}/folder/set"),
    }), device, &avail).await;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slug_for_entity_ids() {
        assert_eq!(slug("Haze PIXEL 8"), "haze_pixel_8");
        assert_eq!(slug("  Haze -- Living "), "haze_living");
    }

    #[test]
    fn folders_with_the_same_name_get_their_parent() {
        let opts = folder_options(["Music/Rock", "Download/Rock", "Music/Jazz", "Music/Rock"]);
        let names: Vec<&str> = opts.iter().map(|(n, _)| n.as_str()).collect();
        assert_eq!(names, [ALL_SONGS, "Rock", "Jazz", "Music / Rock"]);
        assert_eq!(opts[3].1, "Music/Rock");
    }

    #[test]
    fn incoming_messages() {
        let folders = folder_options(["Music/Rock"]);
        assert_eq!(parse("cmd", "next", &folders), Some(Command::Next));
        assert_eq!(parse("volume/set", "40", &folders), Some(Command::Volume(40.0)));
        assert_eq!(parse("repeat/set", "Una", &folders), Some(Command::Repeat("one".into())));
        assert_eq!(parse("shuffle/set", "OFF", &folders), Some(Command::Shuffle(false)));
        assert_eq!(parse("folder/set", "Rock", &folders), Some(Command::PlayFolder("Music/Rock".into())));
        assert_eq!(parse("folder/set", "Pop", &folders), None);
        assert_eq!(parse("cmd", "dance", &folders), None);
    }
}
