//! Bot de Discord: entra al canal de voz donde este el dueño del bot y pone lo mismo que
//! suena en Haze. Los demas del canal suman temas con /play (de YouTube, un link o un
//! archivo suyo) y van a la misma cola: un solo bot, una sola cola, nada choca.
//!
//! Costo: apagado no hay conexion ni tareas. Prendido y sin canal, es una conexion al
//! gateway que late cada ~40 s. El audio (Opus) solo se arma con el bot dentro de un canal.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use haze_core::player::Engine;
use haze_core::song::{id_for, Song, SongKind};
use serde::Serialize;
use serenity::all::{
    ActivityData, Attachment, ChannelId, Client, Command, CommandInteraction, CommandOptionType, Context, CreateCommand,
    CreateCommandOption, CreateInteractionResponse, CreateInteractionResponseMessage, EditInteractionResponse, EventHandler,
    GatewayIntents, Guild, GuildId, Interaction, Ready, ResolvedValue, ShardManager, UserId, VoiceState,
};
use songbird::input::codecs::OpusDecoder;
use songbird::input::{File as FileInput, HttpRequest, Input};
use songbird::tracks::{PlayMode, Track, TrackHandle};
use songbird::{Call, Config as VoiceConfig, Event, EventContext, SerenityInit, Songbird, TrackEvent};
use symphonia::core::audio::{AsAudioBufferRef, AudioBuffer, AudioBufferRef, Channels, SignalSpec};
use symphonia::core::codecs::{CodecDescriptor, CodecParameters, CodecRegistry, Decoder, DecoderOptions, FinalizeResult};
use symphonia::core::errors::Error as SymphError;
use symphonia::core::formats::Packet;
use tauri::{AppHandle, Emitter, Manager};
use tokio::sync::{mpsc, Mutex as AsyncMutex};

use crate::tools::Tool;
use crate::{youtube, AppState};

/// Conectar y hablar en el canal de voz (1 << 20 | 1 << 21), y ver el canal (1 << 10).
const PERMISSIONS: u64 = (1 << 10) | (1 << 20) | (1 << 21);

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    /// off, connecting, online, error
    pub state: String,
    pub error: Option<String>,
    /// Nombre del bot, ya conectado.
    pub bot: Option<String>,
    /// Link para sumar el bot a un servidor.
    pub invite: Option<String>,
    /// Donde esta sonando: "Servidor · canal".
    pub channel: Option<String>,
}

impl Default for Status {
    fn default() -> Self {
        Self { state: "off".into(), error: None, bot: None, invite: None, channel: None }
    }
}

struct Running {
    shards: Arc<ShardManager>,
    voice: Arc<Songbird>,
}

pub struct Discord {
    status: Mutex<Status>,
    run: AsyncMutex<Option<Running>>,
    /// Sube con cada arranque: lo que llegue de un bot anterior se ignora.
    gen: AtomicU64,
    here: Mutex<Option<(GuildId, ChannelId)>>,
    ctx: Mutex<Option<Context>>,
    pub mirror: Arc<Mirror>,
}

impl Discord {
    pub fn new(mute_local: bool) -> Self {
        let mirror = Mirror::default();
        mirror.mute_local.store(mute_local, Ordering::Relaxed);
        Self {
            status: Mutex::new(Status::default()),
            run: AsyncMutex::new(None),
            gen: AtomicU64::new(0),
            here: Mutex::new(None),
            ctx: Mutex::new(None),
            mirror: Arc::new(mirror),
        }
    }
}

fn discord(app: &AppHandle) -> &Discord {
    &app.state::<AppState>().inner().discord
}

pub fn status(app: &AppHandle) -> Status {
    discord(app).status.lock().unwrap().clone()
}

fn set_status(app: &AppHandle, f: impl FnOnce(&mut Status)) {
    let s = {
        let mut st = discord(app).status.lock().unwrap();
        f(&mut st);
        st.clone()
    };
    let _ = app.emit("discord://status", s);
}

fn toast(app: &AppHandle, text: String) {
    let _ = app.emit("app://error", text);
}

// --------------------------------------------------------------------- prender / apagar

/// Prende, apaga o reinicia el bot segun los ajustes.
pub async fn apply(app: &AppHandle) {
    let d = discord(app);
    let cfg = app.state::<AppState>().settings.lock().unwrap().discord.clone();
    let mut run = d.run.lock().await;
    let gen = d.gen.fetch_add(1, Ordering::SeqCst) + 1;
    if let Some(r) = run.take() {
        stop_running(app, r).await;
    }
    let token = cfg.token.trim().to_string();
    if !cfg.enabled || token.is_empty() {
        set_status(app, |s| *s = Status::default());
        return;
    }
    set_status(app, |s| *s = Status { state: "connecting".into(), ..Status::default() });

    let voice = Songbird::serenity_from_config(VoiceConfig::default().codec_registry(codecs()));
    let handler = Handler { app: app.clone(), voice: voice.clone(), gen, owner: Mutex::new(None), moving: AsyncMutex::new(()) };
    // Solo hace falta saber de servidores y de quien esta en que canal de voz.
    let mut cache = serenity::cache::Settings::default();
    cache.max_messages = 0;
    cache.cache_users = false;
    let intents = GatewayIntents::GUILDS | GatewayIntents::GUILD_VOICE_STATES;
    let built = Client::builder(&token, intents)
        .event_handler(handler)
        .register_songbird_with(voice.clone())
        .cache_settings(cache)
        .await;
    let mut client = match built {
        Ok(c) => c,
        Err(e) => {
            set_status(app, |s| *s = Status { state: "error".into(), error: Some(explain(&e)), ..Status::default() });
            return;
        }
    };
    let shards = client.shard_manager.clone();
    let app2 = app.clone();
    tauri::async_runtime::spawn(async move {
        let result = client.start().await;
        if discord(&app2).gen.load(Ordering::SeqCst) != gen {
            return;
        }
        if let Err(e) = result {
            set_status(&app2, |s| *s = Status { state: "error".into(), error: Some(explain(&e)), ..Status::default() });
        }
    });
    *run = Some(Running { shards, voice });
}

async fn stop_running(app: &AppHandle, r: Running) {
    let d = discord(app);
    let here = d.here.lock().unwrap().take();
    if let Some((g, _)) = here {
        let _ = r.voice.remove(g).await;
    }
    d.mirror.detach();
    *d.ctx.lock().unwrap() = None;
    refresh_local_volume(app).await;
    r.shards.shutdown_all().await;
}

/// Al salir de la app: que el bot deje el canal en vez de quedar colgado ahi.
pub fn shutdown(app: &AppHandle) {
    if discord(app).run.try_lock().is_ok_and(|r| r.is_none()) {
        return;
    }
    let app = app.clone();
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        tauri::async_runtime::block_on(async {
            let d = discord(&app);
            d.gen.fetch_add(1, Ordering::SeqCst);
            if let Some(r) = d.run.lock().await.take() {
                stop_running(&app, r).await;
            }
        });
        let _ = tx.send(());
    });
    let _ = rx.recv_timeout(Duration::from_millis(1500));
}

fn explain(e: &serenity::Error) -> String {
    use serenity::gateway::GatewayError as G;
    match e {
        serenity::Error::Gateway(G::InvalidAuthentication) => "El token no es válido".into(),
        serenity::Error::Gateway(G::DisallowedGatewayIntents | G::InvalidGatewayIntents) => {
            "Discord no dejó usar los permisos del bot".into()
        }
        other => other.to_string(),
    }
}

/// Vuelve a aplicar el volumen: los parlantes de la PC se silencian (o no) segun el bot
/// este en un canal y el ajuste.
pub async fn refresh_local_volume(app: &AppHandle) {
    let state = app.state::<AppState>();
    let mut p = state.player.lock().await;
    let v = p.state().volume;
    p.set_volume(v);
}

// --------------------------------------------------------------------- eventos de Discord

struct Handler {
    app: AppHandle,
    voice: Arc<Songbird>,
    gen: u64,
    /// El dueño del bot: a el lo sigue de canal en canal.
    owner: Mutex<Option<UserId>>,
    /// Entrar y salir de canales, de a uno.
    moving: AsyncMutex<()>,
}

impl Handler {
    fn live(&self) -> bool {
        discord(&self.app).gen.load(Ordering::SeqCst) == self.gen
    }

    fn owner(&self) -> Option<UserId> {
        *self.owner.lock().unwrap()
    }

    async fn join(&self, ctx: &Context, g: GuildId, ch: ChannelId) {
        let _turn = self.moving.lock().await;
        let d = discord(&self.app);
        let prev = *d.here.lock().unwrap();
        if prev == Some((g, ch)) {
            return;
        }
        if let Some((pg, _)) = prev.filter(|(pg, _)| *pg != g) {
            let _ = self.voice.remove(pg).await;
            d.mirror.detach();
            *d.here.lock().unwrap() = None;
        }
        let call = match self.voice.join(g, ch).await {
            Ok(c) => c,
            Err(e) => {
                toast(&self.app, format!("Discord: no se pudo entrar al canal ({e})"));
                return;
            }
        };
        // No escucha a nadie: menos trafico.
        let _ = call.lock().await.deafen(true).await;
        *d.here.lock().unwrap() = Some((g, ch));
        let place = place_name(ctx, g, ch);
        set_status(&self.app, |s| s.channel = Some(place));
        if !d.mirror.attached() {
            d.mirror.attach(&self.app, call);
            sync(&self.app).await;
        }
        refresh_local_volume(&self.app).await;
    }

    async fn leave(&self, g: GuildId) {
        let _ = self.voice.remove(g).await;
        self.left(g).await;
    }

    /// El bot ya no esta en ese servidor (salio solo o lo sacaron).
    async fn left(&self, g: GuildId) {
        let d = discord(&self.app);
        let was = {
            let mut here = d.here.lock().unwrap();
            if here.is_some_and(|(hg, _)| hg == g) {
                here.take()
            } else {
                None
            }
        };
        if was.is_none() {
            return;
        }
        let _ = self.voice.remove(g).await;
        d.mirror.detach();
        if let Some(ctx) = d.ctx.lock().unwrap().as_ref() {
            ctx.set_activity(None);
        }
        set_status(&self.app, |s| s.channel = None);
        refresh_local_volume(&self.app).await;
    }

    /// El canal de voz donde esta el dueño, en algun servidor donde este el bot.
    fn owner_channel(&self, ctx: &Context) -> Option<(GuildId, ChannelId)> {
        let owner = self.owner()?;
        ctx.cache.guilds().into_iter().find_map(|g| {
            let ch = ctx.cache.guild(g)?.voice_states.get(&owner)?.channel_id?;
            Some((g, ch))
        })
    }
}

#[serenity::async_trait]
impl EventHandler for Handler {
    async fn ready(&self, ctx: Context, ready: Ready) {
        if !self.live() {
            return;
        }
        let owner = match ctx.http.get_current_application_info().await {
            Ok(info) => info.team.map(|t| t.owner_user_id).or(info.owner.map(|u| u.id)),
            Err(e) => {
                log::warn!("discord: no se pudo leer la aplicacion: {e}");
                None
            }
        };
        *self.owner.lock().unwrap() = owner;
        *discord(&self.app).ctx.lock().unwrap() = Some(ctx.clone());
        let invite = format!(
            "https://discord.com/oauth2/authorize?client_id={}&scope=bot%20applications.commands&permissions={PERMISSIONS}",
            ready.application.id
        );
        let name = ready.user.name.clone();
        set_status(&self.app, |s| {
            s.state = "online".into();
            s.error = owner.is_none().then(|| "No se pudo saber quién es el dueño del bot".into());
            s.bot = Some(name);
            s.invite = Some(invite);
        });
        if let Err(e) = Command::set_global_commands(&ctx.http, commands()).await {
            log::warn!("discord: no se pudieron registrar los comandos: {e}");
        }
        if let Some((g, ch)) = self.owner_channel(&ctx) {
            self.join(&ctx, g, ch).await;
        }
    }

    async fn guild_create(&self, ctx: Context, guild: Guild, _is_new: Option<bool>) {
        if !self.live() {
            return;
        }
        let Some(owner) = self.owner() else { return };
        if let Some(ch) = guild.voice_states.get(&owner).and_then(|v| v.channel_id) {
            self.join(&ctx, guild.id, ch).await;
        }
    }

    async fn voice_state_update(&self, ctx: Context, _old: Option<VoiceState>, new: VoiceState) {
        if !self.live() {
            return;
        }
        let Some(g) = new.guild_id else { return };
        if new.user_id == ctx.cache.current_user().id {
            match new.channel_id {
                None => self.left(g).await,
                // Alguien lo movio de canal: sigue sonando ahi.
                Some(ch) => {
                    let d = discord(&self.app);
                    let moved = {
                        let mut here = d.here.lock().unwrap();
                        match *here {
                            Some((hg, hc)) if hg == g && hc != ch => {
                                *here = Some((g, ch));
                                true
                            }
                            _ => false,
                        }
                    };
                    if moved {
                        let place = place_name(&ctx, g, ch);
                        set_status(&self.app, |s| s.channel = Some(place));
                    }
                }
            }
            return;
        }
        if Some(new.user_id) != self.owner() {
            return;
        }
        match new.channel_id {
            Some(ch) => self.join(&ctx, g, ch).await,
            None => {
                let here = *discord(&self.app).here.lock().unwrap();
                if here.is_some_and(|(hg, _)| hg == g) {
                    self.leave(g).await;
                }
            }
        }
    }

    async fn interaction_create(&self, ctx: Context, interaction: Interaction) {
        if !self.live() {
            return;
        }
        if let Interaction::Command(cmd) = interaction {
            self.command(&ctx, cmd).await;
        }
    }
}

fn place_name(ctx: &Context, g: GuildId, ch: ChannelId) -> String {
    ctx.cache
        .guild(g)
        .map(|gd| {
            let channel = gd.channels.get(&ch).map(|c| c.name.clone()).unwrap_or_else(|| "canal de voz".into());
            format!("{} · {channel}", gd.name)
        })
        .unwrap_or_else(|| "un canal de voz".into())
}

// --------------------------------------------------------------------- comandos

fn commands() -> Vec<CreateCommand> {
    vec![
        // Obligatorias: Discord las deja elegidas y se escribe "/play bohemian rhapsody" directo.
        CreateCommand::new("play").description("Busca en YouTube (o abre un link) y lo suma a la cola de Haze").add_option(
            CreateCommandOption::new(CommandOptionType::String, "tema", "Qué buscar en YouTube, o un link").required(true),
        ),
        CreateCommand::new("subir").description("Suma un archivo de audio tuyo a la cola de Haze").add_option(
            CreateCommandOption::new(CommandOptionType::Attachment, "archivo", "El archivo de audio").required(true),
        ),
        CreateCommand::new("saltar").description("Pasa al tema siguiente"),
        CreateCommand::new("pausa").description("Pausa o sigue"),
        CreateCommand::new("cola").description("Lo que viene en la cola"),
        CreateCommand::new("sonando").description("Qué está sonando"),
    ]
}

async fn reply(ctx: &Context, cmd: &CommandInteraction, text: impl Into<String>, private: bool) {
    let msg = CreateInteractionResponseMessage::new().content(text).ephemeral(private);
    let _ = cmd.create_response(&ctx.http, CreateInteractionResponse::Message(msg)).await;
}

fn describe(song: &Song) -> String {
    if song.artist.is_empty() {
        format!("**{}**", song.title)
    } else {
        format!("**{}** — {}", song.title, song.artist)
    }
}

impl Handler {
    async fn command(&self, ctx: &Context, cmd: CommandInteraction) {
        // Solo los que estan en el mismo canal que el bot.
        let here = *discord(&self.app).here.lock().unwrap();
        let in_channel = match (here, cmd.guild_id) {
            (Some((g, ch)), Some(cg)) if g == cg => {
                let user_ch = ctx.cache.guild(g).and_then(|gd| gd.voice_states.get(&cmd.user.id).and_then(|v| v.channel_id));
                user_ch == Some(ch)
            }
            _ => false,
        };
        if !in_channel {
            let msg = if here.is_none() { "Haze no está en ningún canal de voz." } else { "Entrá al canal de voz donde está Haze." };
            reply(ctx, &cmd, msg, true).await;
            return;
        }
        let state = self.app.state::<AppState>();
        match cmd.data.name.as_str() {
            "play" | "subir" => self.play(ctx, &cmd).await,
            "saltar" => {
                state.player.lock().await.next();
                reply(ctx, &cmd, "⏭ Siguiente", false).await;
            }
            "pausa" => {
                let playing = {
                    let mut p = state.player.lock().await;
                    p.toggle();
                    p.state().playing
                };
                reply(ctx, &cmd, if playing { "▶ Sigue" } else { "⏸ En pausa" }, false).await;
            }
            "cola" => {
                let q = state.player.lock().await.queue_view();
                let next: Vec<String> = q
                    .entries
                    .iter()
                    .skip(q.current_index.map_or(0, |c| c + 1))
                    .take(10)
                    .enumerate()
                    .map(|(i, e)| format!("{}. {}", i + 1, describe(&e.song)))
                    .collect();
                let text = if next.is_empty() { "No viene nada más.".to_string() } else { next.join("\n") };
                reply(ctx, &cmd, text, true).await;
            }
            "sonando" => {
                let song = state.player.lock().await.state().song;
                let text = song.map(|s| format!("🎵 {}", describe(&s))).unwrap_or_else(|| "No suena nada.".into());
                reply(ctx, &cmd, text, true).await;
            }
            _ => {}
        }
    }

    async fn play(&self, ctx: &Context, cmd: &CommandInteraction) {
        // Buscar en YouTube tarda mas que los 3 s que da Discord para contestar.
        let _ = cmd.defer(&ctx.http).await;
        let who = cmd.member.as_ref().and_then(|m| m.nick.clone()).unwrap_or_else(|| cmd.user.display_name().to_string());
        let mut tema = None;
        let mut archivo = None;
        for o in cmd.data.options() {
            match o.value {
                ResolvedValue::String(s) => tema = Some(s.trim().to_string()),
                ResolvedValue::Attachment(a) => archivo = Some(a.clone()),
                _ => {}
            }
        }
        let found = match (archivo, tema) {
            (Some(a), _) => attachment_song(&a).map(|s| vec![s]),
            (None, Some(t)) if !t.is_empty() => find(&self.app, &t).await,
            _ => Err("Decime qué poner: un tema, un link o un archivo.".into()),
        };
        let text = match found {
            Ok(songs) if !songs.is_empty() => {
                let state = self.app.state::<AppState>();
                let mut p = state.player.lock().await;
                for s in &songs {
                    p.add_to_queue(Arc::new(s.clone()));
                }
                if songs.len() == 1 {
                    format!("🎵 {} a la cola — pedido por {who}", describe(&songs[0]))
                } else {
                    format!("🎵 {} temas a la cola — pedido por {who}", songs.len())
                }
            }
            Ok(_) => "No encontré nada.".into(),
            Err(e) => format!("No se pudo: {e}"),
        };
        let _ = cmd.edit_response(&ctx.http, EditInteractionResponse::new().content(text)).await;
    }
}

fn attachment_song(a: &Attachment) -> Result<Song, String> {
    let is_audio = a.content_type.as_deref().is_some_and(|t| t.starts_with("audio/") || t.starts_with("video/"));
    if !is_audio && !direct_media(&a.filename) {
        return Err("Ese archivo no es de audio.".into());
    }
    let title = a.filename.rsplit_once('.').map_or(a.filename.as_str(), |(stem, _)| stem).replace('_', " ");
    Ok(Song {
        id: id_for(&a.url),
        path: a.url.clone(),
        title,
        artist: String::new(),
        album: String::new(),
        duration_ms: 0,
        mtime: 0,
        folder: "Discord".into(),
        kind: SongKind::Stream,
        thumb: None,
        has_cover: false,
        file: 0,
    })
}

/// Un link (video o playlist) o el primer resultado de buscar en YouTube.
async fn find(app: &AppHandle, q: &str) -> Result<Vec<Song>, String> {
    youtube::ensure_tool(app, Tool::YtDlp).await?;
    youtube::ensure_tool(app, Tool::Deno).await?;
    if q.starts_with("http://") || q.starts_with("https://") {
        Ok(youtube::inspect(app, q).await?.songs.into_iter().take(100).collect())
    } else {
        Ok(youtube::search(app, q).await?.into_iter().take(1).collect())
    }
}

// --------------------------------------------------------------------- audio

enum Cmd {
    Load(Song, f64, bool),
    Pause(bool),
    Seek(f64),
    Volume(f64),
    Stop,
}

/// Repite en el canal de voz lo que hace el motor de la PC. Sin canal no hace nada.
#[derive(Default)]
pub struct Mirror {
    tx: Mutex<Option<mpsc::UnboundedSender<Cmd>>>,
    mute_local: AtomicBool,
}

impl Mirror {
    pub fn set_mute_local(&self, on: bool) {
        self.mute_local.store(on, Ordering::Relaxed);
    }

    fn attached(&self) -> bool {
        self.tx.lock().unwrap().is_some()
    }

    fn send(&self, c: Cmd) {
        if let Some(tx) = self.tx.lock().unwrap().as_ref() {
            let _ = tx.send(c);
        }
    }

    fn attach(&self, app: &AppHandle, call: Arc<AsyncMutex<Call>>) {
        let (tx, rx) = mpsc::unbounded_channel();
        *self.tx.lock().unwrap() = Some(tx);
        let worker = Worker { app: app.clone(), call, client: reqwest::Client::new(), track: None, volume: 0.7 };
        tauri::async_runtime::spawn(worker.run(rx));
    }

    /// Cierra el canal: el que reproduce para el tema y termina.
    fn detach(&self) {
        self.tx.lock().unwrap().take();
    }

    /// Volumen para los parlantes de la PC.
    fn local_volume(&self, v: f64) -> f64 {
        if self.mute_local.load(Ordering::Relaxed) && self.attached() {
            0.0
        } else {
            v
        }
    }
}

/// Le manda lo que suena al canal, si el bot esta en uno.
async fn sync(app: &AppHandle) {
    let state = app.state::<AppState>();
    let p = state.player.lock().await;
    let st = p.state();
    let mirror = &state.discord.mirror;
    mirror.send(Cmd::Volume(st.volume));
    if let Some(song) = st.song {
        mirror.send(Cmd::Load((*song).clone(), p.position(), !st.playing));
    }
}

/// El motor de la PC con el canal de voz atras: todo lo que hace uno lo repite el otro.
/// El tiempo, el fin del tema y los errores siguen saliendo de la PC.
pub struct Tee<E> {
    pub local: E,
    pub mirror: Arc<Mirror>,
}

impl<E: Engine> Engine for Tee<E> {
    fn load(&self, song: &Song, start: f64, paused: bool) {
        self.local.load(song, start, paused);
        self.mirror.send(Cmd::Load(song.clone(), start, paused));
    }

    fn set_paused(&self, paused: bool) {
        self.local.set_paused(paused);
        self.mirror.send(Cmd::Pause(paused));
    }

    fn seek(&self, seconds: f64) {
        self.local.seek(seconds);
        self.mirror.send(Cmd::Seek(seconds));
    }

    fn set_volume(&self, volume: f64) {
        self.local.set_volume(self.mirror.local_volume(volume));
        self.mirror.send(Cmd::Volume(volume));
    }

    fn stop(&self) {
        self.local.stop();
        self.mirror.send(Cmd::Stop);
    }

    fn reopen_audio(&self) {
        self.local.reopen_audio();
    }
}

struct Worker {
    app: AppHandle,
    call: Arc<AsyncMutex<Call>>,
    client: reqwest::Client,
    track: Option<TrackHandle>,
    volume: f32,
}

impl Worker {
    async fn run(mut self, mut rx: mpsc::UnboundedReceiver<Cmd>) {
        while let Some(first) = rx.recv().await {
            let mut batch = vec![first];
            while let Ok(c) = rx.try_recv() {
                batch.push(c);
            }
            // Saltando temas rapido se juntan cargas: solo importa la ultima (y el volumen).
            if let Some(last) = batch.iter().rposition(|c| matches!(c, Cmd::Load(..))) {
                for c in batch.drain(..last) {
                    if let Cmd::Volume(v) = c {
                        self.set_volume(v);
                    }
                }
            }
            for c in batch {
                self.handle(c).await;
            }
        }
        if let Some(t) = self.track.take() {
            let _ = t.stop();
        }
    }

    fn set_volume(&mut self, v: f64) {
        self.volume = (v / 100.0).clamp(0.0, 1.0) as f32;
        if let Some(t) = &self.track {
            let _ = t.set_volume(self.volume);
        }
    }

    async fn handle(&mut self, c: Cmd) {
        match c {
            Cmd::Load(song, start, paused) => self.load(song, start, paused).await,
            Cmd::Pause(p) => {
                if let Some(t) = &self.track {
                    let _ = if p { t.pause() } else { t.play() };
                }
            }
            Cmd::Seek(s) => {
                if let Some(t) = &self.track {
                    let _ = t.seek(Duration::from_secs_f64(s.max(0.0)));
                }
            }
            Cmd::Volume(v) => self.set_volume(v),
            Cmd::Stop => {
                if let Some(t) = self.track.take() {
                    let _ = t.stop();
                }
            }
        }
    }

    async fn load(&mut self, song: Song, start: f64, paused: bool) {
        if let Some(t) = self.track.take() {
            let _ = t.stop();
        }
        let asked = Instant::now();
        let input = match input_for(&self.app, &self.client, &song).await {
            Ok(i) => i,
            Err(e) => {
                toast(&self.app, format!("Discord: no se pudo poner \"{}\": {e}", song.title));
                return;
            }
        };
        // Mientras se buscaba el audio, la PC ya arranco: se adelanta lo mismo para ir parejos.
        let start = if paused { start } else { start + asked.elapsed().as_secs_f64() };
        let mut track = Track::new(input).volume(self.volume);
        if paused {
            track = track.pause();
        }
        let handle = self.call.lock().await.play_only(track);
        if start > 0.5 {
            let _ = handle.seek(Duration::from_secs_f64(start));
        }
        let _ = handle.add_event(Event::Track(TrackEvent::Error), TrackFailed(self.app.clone(), song.title.clone()));
        if let Some(ctx) = discord(&self.app).ctx.lock().unwrap().as_ref() {
            let mut text = if song.artist.is_empty() { song.title.clone() } else { format!("{} — {}", song.title, song.artist) };
            text.truncate(text.char_indices().nth(120).map_or(text.len(), |(i, _)| i));
            ctx.set_activity(Some(ActivityData::listening(text)));
        }
        self.track = Some(handle);
    }
}

/// Archivos de la PC tal cual; links directos (adjuntos de Discord) por HTTP; lo demas
/// (YouTube, SoundCloud...) se resuelve con yt-dlp.
async fn input_for(app: &AppHandle, client: &reqwest::Client, song: &Song) -> Result<Input, String> {
    if song.kind == SongKind::Local {
        return Ok(FileInput::new(song.path.clone()).into());
    }
    let (url, headers) = if direct_media(&song.path) {
        (song.path.clone(), vec![])
    } else {
        youtube::direct_audio(app, &song.path).await?
    };
    let mut map = reqwest::header::HeaderMap::new();
    for (k, v) in headers {
        if let (Ok(k), Ok(v)) = (reqwest::header::HeaderName::from_bytes(k.as_bytes()), reqwest::header::HeaderValue::from_str(&v)) {
            map.insert(k, v);
        }
    }
    Ok(HttpRequest::new_with_headers(client.clone(), url, map).into())
}

fn direct_media(url: &str) -> bool {
    let path = url.split(['?', '#']).next().unwrap_or(url).to_lowercase();
    url.contains("cdn.discordapp.com")
        || url.contains("media.discordapp.net")
        || [".mp3", ".m4a", ".flac", ".ogg", ".opus", ".wav", ".aac", ".webm"].iter().any(|e| path.ends_with(e))
}

/// Decodificadores para el canal de voz. Songbird corta el tema entero al primer paquete
/// que no se puede decodificar, y hay MP3 bajados de por ahi con cuadros rotos (sobre todo
/// al principio) que la PC saltea sin que se note. Aca se hace lo mismo.
fn codecs() -> &'static CodecRegistry {
    static REGISTRY: OnceLock<CodecRegistry> = OnceLock::new();
    REGISTRY.get_or_init(|| {
        use symphonia::default::codecs::{AacDecoder, AlacDecoder, FlacDecoder, MpaDecoder, VorbisDecoder};
        let mut r = CodecRegistry::new();
        symphonia::default::register_enabled_codecs(&mut r);
        r.register_all::<OpusDecoder>();
        tolerate::<MpaDecoder>(&mut r);
        tolerate::<AacDecoder>(&mut r);
        tolerate::<AlacDecoder>(&mut r);
        tolerate::<FlacDecoder>(&mut r);
        tolerate::<VorbisDecoder>(&mut r);
        tolerate::<OpusDecoder>(&mut r);
        r
    })
}

/// Reemplaza los codecs de `D` por la version que saltea paquetes rotos.
fn tolerate<D: Decoder + 'static>(r: &mut CodecRegistry) {
    fn make<D: Decoder + 'static>(p: &CodecParameters, o: &DecoderOptions) -> symphonia::core::errors::Result<Box<dyn Decoder>> {
        Ok(Box::new(Tolerant::new(Box::new(D::try_new(p, o)?))))
    }
    for d in D::supported_codecs() {
        r.register(&CodecDescriptor { inst_func: make::<D>, ..*d });
    }
}

/// Paquetes rotos seguidos que se aguantan antes de dar el tema por perdido (~5 s de MP3).
const MAX_BAD_PACKETS: u32 = 200;

struct Tolerant {
    inner: Box<dyn Decoder>,
    /// Lo que se devuelve por un paquete roto: nada, y songbird pasa al siguiente.
    empty: AudioBuffer<f32>,
    bad: u32,
}

impl Tolerant {
    fn new(inner: Box<dyn Decoder>) -> Self {
        let p = inner.codec_params();
        let rate = p.sample_rate.unwrap_or(48_000);
        let channels = p.channels.unwrap_or(Channels::FRONT_LEFT | Channels::FRONT_RIGHT);
        Self { inner, empty: AudioBuffer::new(0, SignalSpec::new(rate, channels)), bad: 0 }
    }
}

impl Decoder for Tolerant {
    fn try_new(_: &CodecParameters, _: &DecoderOptions) -> symphonia::core::errors::Result<Self> {
        Err(SymphError::Unsupported("se arma con tolerate()"))
    }

    fn supported_codecs() -> &'static [CodecDescriptor] {
        &[]
    }

    fn reset(&mut self) {
        self.bad = 0;
        self.inner.reset();
    }

    fn codec_params(&self) -> &CodecParameters {
        self.inner.codec_params()
    }

    fn decode(&mut self, packet: &Packet) -> symphonia::core::errors::Result<AudioBufferRef<'_>> {
        match self.inner.decode(packet) {
            Ok(_) => self.bad = 0,
            Err(SymphError::DecodeError(e)) if self.bad < MAX_BAD_PACKETS => {
                if self.bad == 0 {
                    log::debug!("discord: paquete roto salteado ({e})");
                }
                self.bad += 1;
                return Ok(self.empty.as_audio_buffer_ref());
            }
            Err(e) => return Err(e),
        }
        Ok(self.inner.last_decoded())
    }

    fn finalize(&mut self) -> FinalizeResult {
        self.inner.finalize()
    }

    fn last_decoded(&self) -> AudioBufferRef<'_> {
        if self.bad > 0 {
            self.empty.as_audio_buffer_ref()
        } else {
            self.inner.last_decoded()
        }
    }
}

struct TrackFailed(AppHandle, String);

#[serenity::async_trait]
impl songbird::EventHandler for TrackFailed {
    async fn act(&self, ctx: &EventContext<'_>) -> Option<Event> {
        if let EventContext::Track(list) = ctx {
            for (state, _) in *list {
                if let PlayMode::Errored(e) = &state.playing {
                    toast(&self.0, format!("Discord: falló \"{}\": {e}", self.1));
                }
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::{codecs, direct_media, Tolerant, MAX_BAD_PACKETS};
    use symphonia::core::audio::AudioBufferRef;
    use symphonia::core::codecs::{CodecDescriptor, CodecParameters, Decoder, DecoderOptions, FinalizeResult};
    use symphonia::core::errors::{decode_error, Result};
    use symphonia::core::formats::{FormatOptions, Packet};
    use symphonia::core::io::MediaSourceStream;

    /// Un decodificador al que todo paquete le llega roto.
    struct Broken(CodecParameters);

    impl Decoder for Broken {
        fn try_new(p: &CodecParameters, _: &DecoderOptions) -> Result<Self> {
            Ok(Self(p.clone()))
        }
        fn supported_codecs() -> &'static [CodecDescriptor] {
            &[]
        }
        fn reset(&mut self) {}
        fn codec_params(&self) -> &CodecParameters {
            &self.0
        }
        fn decode(&mut self, _: &Packet) -> Result<AudioBufferRef<'_>> {
            decode_error("roto")
        }
        fn finalize(&mut self) -> FinalizeResult {
            FinalizeResult::default()
        }
        fn last_decoded(&self) -> AudioBufferRef<'_> {
            unreachable!()
        }
    }

    #[test]
    fn broken_packets_are_skipped() {
        let mut dec = Tolerant::new(Box::new(Broken(CodecParameters::new())));
        let pkt = Packet::new_from_slice(0, 0, 1152, &[0; 8]);
        for _ in 0..MAX_BAD_PACKETS {
            assert_eq!(dec.decode(&pkt).expect("se saltea").frames(), 0);
        }
        assert!(dec.decode(&pkt).is_err(), "demasiados seguidos: se rinde");
    }

    /// Con un MP3 de cuadros rotos: `HAZE_BROKEN_MP3=ruta cargo test -- --ignored`.
    #[test]
    #[ignore]
    fn real_broken_mp3_plays_through() {
        let path = std::env::var("HAZE_BROKEN_MP3").expect("falta HAZE_BROKEN_MP3");
        let mss = MediaSourceStream::new(Box::new(std::fs::File::open(path).unwrap()), Default::default());
        let probed = songbird::input::codecs::get_probe()
            .format(&Default::default(), mss, &FormatOptions::default(), &Default::default())
            .unwrap();
        let mut format = probed.format;
        let track = format.default_track().unwrap().clone();
        let mut dec = codecs().make(&track.codec_params, &DecoderOptions::default()).unwrap();
        let (mut frames, mut packets) = (0, 0);
        while let Ok(pkt) = format.next_packet() {
            frames += dec.decode(&pkt).expect("ningun paquete corta el tema").frames();
            packets += 1;
        }
        assert!(packets > 0 && frames > 0);
    }

    #[test]
    fn direct_links_skip_ytdlp() {
        assert!(direct_media("https://cdn.discordapp.com/attachments/1/2/tema.mp3?ex=abc"));
        assert!(direct_media("https://example.com/a/b.FLAC"));
        assert!(!direct_media("https://www.youtube.com/watch?v=abc"));
        assert!(!direct_media("https://soundcloud.com/x/y"));
    }
}
