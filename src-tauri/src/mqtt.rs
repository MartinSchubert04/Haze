//! Home Assistant por MQTT. Con MQTT discovery las entidades aparecen solas en HA,
//! agrupadas en un dispositivo: estado, cancion, artista, caratula, botones de
//! anterior/play/siguiente, volumen, aleatorio, repetir y "reproducir carpeta".

use std::time::Duration;

use rumqttc::{AsyncClient, Event, LastWill, MqttOptions, Packet, QoS};
use serde_json::json;
use tauri::async_runtime::JoinHandle;
use tokio::sync::watch;

use crate::player::PlayerState;
use crate::settings::HomeAssistant;
use crate::Control;

const REPEAT: &[(&str, &str)] = &[("off", "No"), ("all", "Todo"), ("one", "Una")];

pub struct Bridge {
    task: Option<JoinHandle<()>>,
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
        cfg: HomeAssistant,
        rx: watch::Receiver<PlayerState>,
        folders: impl Fn() -> Vec<(String, String)> + Send + Sync + 'static,
        control: impl Fn(Control) + Send + Sync + 'static,
        status: impl Fn(&str) + Send + Sync + 'static,
    ) {
        self.stop();
        if !cfg.enabled || cfg.host.trim().is_empty() {
            status("off");
            return;
        }
        self.task = Some(tauri::async_runtime::spawn(run(cfg, rx, folders, control, status)));
    }
}

async fn run(
    cfg: HomeAssistant,
    mut rx: watch::Receiver<PlayerState>,
    folders: impl Fn() -> Vec<(String, String)> + Send + Sync + 'static,
    control: impl Fn(Control) + Send + Sync + 'static,
    status: impl Fn(&str) + Send + Sync + 'static,
) {
    let node = slug(&cfg.device_name);
    let base = format!("haze/{node}");
    let avail = format!("{base}/availability");

    let mut opts = MqttOptions::new(format!("haze-{node}"), cfg.host.trim(), cfg.port);
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
    let folder_list = std::sync::Arc::new(std::sync::Mutex::new(Vec::<(String, String)>::new()));
    let folder_list2 = folder_list.clone();
    let poll = tauri::async_runtime::spawn(async move {
        loop {
            match events.poll().await {
                Ok(Event::Incoming(Packet::ConnAck(_))) => {
                    status("connected");
                    let _ = c2.subscribe(format!("{base2}/+/set"), QoS::AtLeastOnce).await;
                    let _ = c2.subscribe(format!("{base2}/cmd"), QoS::AtLeastOnce).await;
                    let _ = c2.subscribe("homeassistant/status", QoS::AtLeastOnce).await;
                    let _ = online_tx.send(());
                }
                Ok(Event::Incoming(Packet::Publish(p))) => {
                    let payload = String::from_utf8_lossy(&p.payload).trim().to_string();
                    let topic = p.topic.strip_prefix(&format!("{base2}/")).unwrap_or(&p.topic).to_string();
                    let c = match topic.as_str() {
                        "cmd" => match payload.as_str() {
                            "toggle" => Some(Control::Toggle),
                            "play" => Some(Control::Play),
                            "pause" => Some(Control::Pause),
                            "next" => Some(Control::Next),
                            "previous" => Some(Control::Previous),
                            _ => None,
                        },
                        "volume/set" => payload.parse().ok().map(Control::Volume),
                        "seek/set" => payload.parse().ok().map(Control::Seek),
                        "shuffle/set" => Some(Control::Shuffle(payload == "ON")),
                        "repeat/set" => REPEAT.iter().find(|(_, l)| *l == payload).map(|(k, _)| Control::Repeat(k.to_string())),
                        "folder/set" => {
                            let list = folder_list2.lock().unwrap();
                            list.iter().find(|(name, _)| *name == payload).map(|(_, path)| Control::PlayFolder(path.clone()))
                        }
                        "homeassistant/status" if payload == "online" => {
                            // HA se reinicio: hay que volver a anunciar las entidades.
                            let _ = online_tx.send(());
                            None
                        }
                        _ => None,
                    };
                    if let Some(c) = c {
                        control(c);
                    }
                }
                Ok(_) => {}
                Err(_) => {
                    status("error");
                    tokio::time::sleep(Duration::from_secs(5)).await;
                }
            }
        }
    });

    let device = json!({
        "identifiers": [format!("haze_{node}")],
        "name": cfg.device_name,
        "manufacturer": "MRTN",
        "model": "Haze (escritorio)",
        "sw_version": env!("CARGO_PKG_VERSION"),
    });
    let state_topic = format!("{base}/state");

    let mut last_cover: Option<u64> = None;
    let mut last_folders: Vec<(String, String)> = Vec::new();

    loop {
        tokio::select! {
            r = rx.changed() => { if r.is_err() { break; } }
            Some(()) = online_rx.recv() => {
                // Anuncio (o re-anuncio) de las entidades.
                last_folders = folders();
                *folder_list.lock().unwrap() = last_folders.clone();
                announce(&client, &base, &avail, &state_topic, &device, &last_folders).await;
                let _ = client.publish(&avail, QoS::AtLeastOnce, true, "online").await;
                last_cover = None;
            }
        }
        let s = rx.borrow_and_update().clone();

        let f = folders();
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
                    let x = x.clone();
                    tauri::async_runtime::spawn_blocking(move || crate::media_keys::cover_bytes(&x)).await.ok().flatten()
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
        options.push("Todas las canciones");
    }
    publish_config(client, "select", &format!("haze_{node}_carpeta"), json!({
        "name": "Reproducir carpeta", "icon": "mdi:folder-music",
        "options": options, "command_topic": format!("{base}/folder/set"),
    }), device, &avail).await;
}
