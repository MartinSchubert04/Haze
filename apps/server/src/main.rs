//! Haze como bot de Discord. Queda prendido en el server y con `/play` se mete en el canal
//! de voz de quien lo pidio. Solo YouTube (o lo que entienda yt-dlp): busca y suena sin
//! bajar nada.
//!
//! La cola es la de `haze-core`: todo lo que se pide con `/play` entra como encolado a
//! mano, asi que lo que ya sono se consume solo y la cola es "la actual + lo que viene".

use std::{collections::HashMap, sync::Arc, time::Duration};

use haze_core::{queue::PlayQueue, song::Song, youtube};
use serenity::{all::*, async_trait};
use songbird::{
    events::{Event as VoiceEvent, EventContext, EventHandler as VoiceEventHandler, TrackEvent},
    input::YoutubeDl,
    tracks::{PlayMode, TrackHandle},
    SerenityInit, Songbird,
};
use tokio::{process::Command as Process, sync::Mutex};
use tracing::{error, info, warn};

/// Sin nada sonando, se va del canal despues de esto.
const IDLE_LEAVE: Duration = Duration::from_secs(5 * 60);
/// Si queda solo en el canal, espera esto por si alguien vuelve.
const ALONE_LEAVE: Duration = Duration::from_secs(30);
/// Tope de canciones por playlist.
const MAX_PLAYLIST: usize = 200;
/// yt-dlp se actualiza cada tanto porque YouTube cambia seguido (igual que en escritorio).
const YTDLP_UPDATE: Duration = Duration::from_secs(3 * 24 * 3600);

struct Player {
    queue: PlayQueue,
    next_uid: u64,
    /// Donde se avisa que tema arranca.
    text: ChannelId,
    track: Option<TrackHandle>,
}

struct Bot {
    http: Arc<Http>,
    voice: Arc<Songbird>,
    web: reqwest::Client,
    players: Mutex<HashMap<GuildId, Player>>,
}

// ---------------------------------------------------------------------------------------
// Reproduccion

/// Pone a sonar la actual de la cola. Devuelve cual.
async fn play_current(bot: &Arc<Bot>, guild: GuildId) -> Option<Arc<Song>> {
    let call = bot.voice.get(guild)?;
    let mut players = bot.players.lock().await;
    let p = players.get_mut(&guild)?;
    let entry = p.queue.current()?.clone();
    let input = YoutubeDl::new(bot.web.clone(), entry.song.path.clone())
        .user_args(vec!["--ignore-config".into(), "--no-warnings".into()]);
    let handle = call.lock().await.play_only_input(input.into());
    for ev in [TrackEvent::End, TrackEvent::Error] {
        let done = TrackDone { bot: bot.clone(), guild, uid: entry.uid };
        if let Err(e) = handle.add_event(VoiceEvent::Track(ev), done) {
            warn!("no pude escuchar el fin del tema: {e}");
        }
    }
    p.track = Some(handle);
    Some(entry.song)
}

/// Termino (o fallo) el tema `uid`: pasa al siguiente si sigue siendo el actual.
async fn advance(bot: &Arc<Bot>, guild: GuildId, uid: u64, failed: bool) {
    let (text, failed_song, has_next) = {
        let mut players = bot.players.lock().await;
        let Some(p) = players.get_mut(&guild) else { return };
        let Some(cur) = p.queue.current().cloned() else { return };
        // Un /skip o /stop ya movio la cola: este aviso llega tarde.
        if cur.uid != uid {
            return;
        }
        p.track = None;
        let next = p.queue.current_index.unwrap_or(0) + 1;
        p.queue = if next < p.queue.entries.len() { p.queue.move_to_index(next) } else { PlayQueue::default() };
        (p.text, failed.then_some(cur.song), p.queue.current().is_some())
    };
    if let Some(song) = failed_song {
        say(bot, text, format!("No pude reproducir {}, sigo con la próxima.", song_link(&song))).await;
    }
    if has_next {
        if let Some(song) = play_current(bot, guild).await {
            say(bot, text, format!("▶ {}", song_line(&song))).await;
        }
    } else {
        leave_when_idle(bot.clone(), guild);
    }
}

struct TrackDone {
    bot: Arc<Bot>,
    guild: GuildId,
    uid: u64,
}

#[async_trait]
impl VoiceEventHandler for TrackDone {
    async fn act(&self, ctx: &EventContext<'_>) -> Option<VoiceEvent> {
        let failed = matches!(ctx, EventContext::Track(ts)
            if ts.iter().any(|(state, _)| matches!(state.playing, PlayMode::Errored(_))));
        if failed {
            warn!("fallo el tema {} en {}", self.uid, self.guild);
        }
        advance(&self.bot, self.guild, self.uid, failed).await;
        None
    }
}

async fn leave(bot: &Arc<Bot>, guild: GuildId) {
    // Primero la cola: asi el fin del tema que corta `remove` no arranca otro.
    bot.players.lock().await.remove(&guild);
    if bot.voice.get(guild).is_some() {
        if let Err(e) = bot.voice.remove(guild).await {
            warn!("no pude salir del canal: {e}");
        }
    }
}

fn leave_when_idle(bot: Arc<Bot>, guild: GuildId) {
    tokio::spawn(async move {
        tokio::time::sleep(IDLE_LEAVE).await;
        let idle = bot.players.lock().await.get(&guild).map_or(true, |p| p.queue.current().is_none());
        if idle && bot.voice.get(guild).is_some() {
            info!("me voy de {guild}: nada sonando");
            leave(&bot, guild).await;
        }
    });
}

async fn say(bot: &Bot, channel: ChannelId, text: String) {
    if let Err(e) = channel.say(&bot.http, text).await {
        warn!("no pude mandar el mensaje: {e}");
    }
}

// ---------------------------------------------------------------------------------------
// YouTube

/// Lo que hay detras de lo que escribieron en /play: el titulo si es playlist y los temas.
async fn resolve(query: &str) -> Result<(Option<String>, Vec<Song>), String> {
    let is_link = query.starts_with("http://") || query.starts_with("https://");
    let mut cmd = Process::new("yt-dlp");
    cmd.args(youtube::JSON_ARGS).kill_on_drop(true);
    if is_link {
        // Un video abierto desde una playlist ("watch?v=..&list=..") es ese video.
        if query.contains("watch?v=") || query.contains("youtu.be/") {
            cmd.arg("--no-playlist");
        }
        cmd.arg(query);
    } else {
        cmd.arg(format!("ytsearch1:{query}"));
    }
    let out = tokio::time::timeout(Duration::from_secs(90), cmd.output())
        .await
        .map_err(|_| "YouTube tardó demasiado en responder".to_string())?
        .map_err(|e| format!("no pude correr yt-dlp: {e}"))?;
    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr);
        return Err(youtube::error_line(&stderr).unwrap_or_else(|| "yt-dlp falló".into()));
    }
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).map_err(|e| e.to_string())?;
    if is_link {
        let info = youtube::link_info(&v)?;
        let mut songs = info.songs;
        songs.truncate(MAX_PLAYLIST);
        Ok((info.is_playlist.then_some(info.title), songs))
    } else {
        Ok((None, youtube::search_results(&v).into_iter().take(1).collect()))
    }
}

async fn keep_ytdlp_updated() {
    loop {
        match Process::new("yt-dlp").arg("-U").output().await {
            Ok(out) => info!("yt-dlp -U: {}", String::from_utf8_lossy(&out.stdout).trim()),
            Err(e) => error!("no pude actualizar yt-dlp: {e}"),
        }
        tokio::time::sleep(YTDLP_UPDATE).await;
    }
}

// ---------------------------------------------------------------------------------------
// Texto

fn duration(ms: u64) -> String {
    let s = ms / 1000;
    if s >= 3600 {
        format!("{}:{:02}:{:02}", s / 3600, s / 60 % 60, s % 60)
    } else {
        format!("{}:{:02}", s / 60, s % 60)
    }
}

/// El titulo con link, sin que Discord arme la vista previa (`<url>`).
fn song_link(song: &Song) -> String {
    let title = song.title.replace(['[', ']'], "");
    format!("[**{title}**](<{}>)", song.path)
}

fn song_line(song: &Song) -> String {
    let mut line = song_link(song);
    if !song.artist.is_empty() {
        line += &format!(" — {}", song.artist);
    }
    if song.duration_ms > 0 {
        line += &format!(" `{}`", duration(song.duration_ms));
    }
    line
}

// ---------------------------------------------------------------------------------------
// Comandos

fn commands() -> Vec<CreateCommand> {
    let simple = |name: &str, desc: &str| CreateCommand::new(name).description(desc);
    vec![
        CreateCommand::new("play")
            .description("Busca en YouTube o pegá un link (video o playlist) y lo pone a sonar")
            .add_option(
                CreateCommandOption::new(CommandOptionType::String, "cancion", "Qué buscar, o un link")
                    .required(true),
            )
            .add_option(CreateCommandOption::new(
                CommandOptionType::Boolean,
                "siguiente",
                "Que suene después de la actual, antes que el resto de la cola",
            )),
        simple("skip", "Pasa a la próxima"),
        simple("pause", "Pausa"),
        simple("resume", "Sigue donde quedó"),
        simple("stop", "Corta y vacía la cola"),
        simple("queue", "Muestra la cola"),
        simple("nowplaying", "Qué está sonando"),
        simple("leave", "Se va del canal de voz"),
    ]
}

async fn reply(ctx: &Context, cmd: &CommandInteraction, text: impl Into<String>) {
    let msg = CreateInteractionResponseMessage::new().content(text);
    if let Err(e) = cmd.create_response(&ctx.http, CreateInteractionResponse::Message(msg)).await {
        warn!("no pude responder: {e}");
    }
}

async fn edit(ctx: &Context, cmd: &CommandInteraction, text: impl Into<String>) {
    if let Err(e) = cmd.edit_response(&ctx.http, EditInteractionResponse::new().content(text)).await {
        warn!("no pude responder: {e}");
    }
}

struct Handler {
    bot: Arc<Bot>,
}

impl Handler {
    async fn play(&self, ctx: &Context, cmd: &CommandInteraction, guild: GuildId) {
        let mut query = String::new();
        let mut next = false;
        for opt in cmd.data.options() {
            match (opt.name, opt.value) {
                ("cancion", ResolvedValue::String(s)) => query = s.trim().to_string(),
                ("siguiente", ResolvedValue::Boolean(b)) => next = b,
                _ => {}
            }
        }
        let channel = ctx
            .cache
            .guild(guild)
            .and_then(|g| g.voice_states.get(&cmd.user.id).and_then(|v| v.channel_id));
        let Some(channel) = channel else {
            return reply(ctx, cmd, "Metete en un canal de voz primero.").await;
        };
        // yt-dlp tarda mas que los 3 segundos que da Discord para contestar.
        if let Err(e) = cmd.defer(&ctx.http).await {
            return warn!("no pude diferir la respuesta: {e}");
        }

        let (playlist, songs) = match resolve(&query).await {
            Ok((_, songs)) if songs.is_empty() => return edit(ctx, cmd, format!("No encontré nada para «{query}».")).await,
            Ok(found) => found,
            Err(e) => return edit(ctx, cmd, format!("No pude buscar eso: {e}")).await,
        };

        let bot = &self.bot;
        let here = match bot.voice.get(guild) {
            Some(call) => call.lock().await.current_channel().map(|c| c.0.get()) == Some(channel.get()),
            None => false,
        };
        if !here {
            match bot.voice.join(guild, channel).await {
                Ok(call) => {
                    // No escucha a nadie: menos trafico.
                    let _ = call.lock().await.deafen(true).await;
                }
                Err(e) => return edit(ctx, cmd, format!("No pude entrar al canal: {e}")).await,
            }
        }

        let (was_idle, position, total, first) = {
            let mut players = bot.players.lock().await;
            let p = players.entry(guild).or_insert_with(|| Player {
                queue: PlayQueue::default(),
                next_uid: 0,
                text: cmd.channel_id,
                track: None,
            });
            p.text = cmd.channel_id;
            let was_idle = p.queue.current().is_none();
            let total = songs.len();
            let songs: Vec<Arc<Song>> = songs.into_iter().map(Arc::new).collect();
            if next && !was_idle {
                // "A continuacion" pone la ultima primero: al reves para respetar el orden.
                for s in songs.iter().rev() {
                    p.queue = p.queue.play_next(s.clone(), p.next_uid);
                    p.next_uid += 1;
                }
            } else {
                for s in &songs {
                    p.queue = p.queue.add_to_queue(s.clone(), p.next_uid);
                    p.next_uid += 1;
                }
            }
            // Cuantos lugares despues de la actual quedo la primera agregada.
            let cur = p.queue.current_index.unwrap_or(0);
            let position = match (was_idle, next) {
                (true, _) => 0,
                (false, true) => 1,
                (false, false) => p.queue.entries.len() - total - cur,
            };
            (was_idle, position, total, songs[0].clone())
        };
        let first_line = song_line(&first);
        let more = if total > 1 { format!("\n…y {} más de la playlist **{}**.", total - 1, playlist.unwrap_or_default()) } else { String::new() };

        if was_idle {
            play_current(&self.bot, guild).await;
            edit(ctx, cmd, format!("▶ {first_line}{more}")).await;
        } else {
            let pos = if next { "suena después de esta".to_string() } else { format!("#{position} en la cola") };
            edit(ctx, cmd, format!("➕ {first_line} ({pos}){more}")).await;
        }
    }

    async fn current_track(&self, guild: GuildId) -> Option<(TrackHandle, Arc<Song>)> {
        let players = self.bot.players.lock().await;
        let p = players.get(&guild)?;
        Some((p.track.clone()?, p.queue.current()?.song.clone()))
    }

    async fn skip(&self, ctx: &Context, cmd: &CommandInteraction, guild: GuildId) {
        match self.current_track(guild).await {
            // El fin del tema hace el resto (ver `advance`).
            Some((track, song)) => {
                let _ = track.stop();
                reply(ctx, cmd, format!("⏭ Salteé {}", song_link(&song))).await;
            }
            None => reply(ctx, cmd, "No está sonando nada.").await,
        }
    }

    async fn pause(&self, ctx: &Context, cmd: &CommandInteraction, guild: GuildId, pause: bool) {
        let Some((track, _)) = self.current_track(guild).await else {
            return reply(ctx, cmd, "No está sonando nada.").await;
        };
        let res = if pause { track.pause() } else { track.play() };
        match res {
            Ok(()) => reply(ctx, cmd, if pause { "⏸ Pausado." } else { "▶ Sigue." }).await,
            Err(e) => reply(ctx, cmd, format!("No pude: {e}")).await,
        }
    }

    async fn stop(&self, ctx: &Context, cmd: &CommandInteraction, guild: GuildId) {
        let track = {
            let mut players = self.bot.players.lock().await;
            players.get_mut(&guild).and_then(|p| {
                p.queue = PlayQueue::default();
                p.track.take()
            })
        };
        if let Some(t) = track {
            let _ = t.stop();
        }
        reply(ctx, cmd, "⏹ Cola vacía.").await;
        leave_when_idle(self.bot.clone(), guild);
    }

    async fn queue(&self, ctx: &Context, cmd: &CommandInteraction, guild: GuildId) {
        let text = {
            let players = self.bot.players.lock().await;
            match players.get(&guild).and_then(|p| Some((p, p.queue.current_index?))) {
                None => "La cola está vacía.".to_string(),
                Some((p, cur)) => {
                    let mut out = format!("**Sonando:** {}\n", song_line(&p.queue.entries[cur].song));
                    let rest = &p.queue.entries[cur + 1..];
                    for (i, e) in rest.iter().take(10).enumerate() {
                        out += &format!("\n`{}.` {}", i + 1, song_line(&e.song));
                    }
                    if rest.len() > 10 {
                        out += &format!("\n…y {} más.", rest.len() - 10);
                    }
                    let total: u64 = rest.iter().map(|e| e.song.duration_ms).sum();
                    if !rest.is_empty() {
                        out += &format!("\n\n{} en la cola, `{}`.", rest.len(), duration(total));
                    }
                    out
                }
            }
        };
        reply(ctx, cmd, text).await;
    }

    async fn now_playing(&self, ctx: &Context, cmd: &CommandInteraction, guild: GuildId) {
        let Some((track, song)) = self.current_track(guild).await else {
            return reply(ctx, cmd, "No está sonando nada.").await;
        };
        let info = track.get_info().await.ok();
        let paused = info.as_ref().is_some_and(|i| matches!(i.playing, PlayMode::Pause));
        let pos = info.map(|i| i.position.as_millis() as u64).unwrap_or(0);
        let mut embed = CreateEmbed::new()
            .title(&song.title)
            .url(&song.path)
            .description(format!(
                "{}{} / {}",
                if paused { "⏸ " } else { "▶ " },
                duration(pos),
                duration(song.duration_ms)
            ));
        if !song.artist.is_empty() {
            embed = embed.author(CreateEmbedAuthor::new(&song.artist));
        }
        if let Some(thumb) = &song.thumb {
            embed = embed.thumbnail(thumb);
        }
        let msg = CreateInteractionResponseMessage::new().embed(embed);
        if let Err(e) = cmd.create_response(&ctx.http, CreateInteractionResponse::Message(msg)).await {
            warn!("no pude responder: {e}");
        }
    }
}

#[async_trait]
impl EventHandler for Handler {
    async fn ready(&self, ctx: Context, ready: Ready) {
        info!("conectado como {}", ready.user.name);
        ctx.set_activity(Some(ActivityData::listening("/play")));
        if let Err(e) = Command::set_global_commands(&ctx.http, commands()).await {
            error!("no pude registrar los comandos: {e}");
        }
    }

    async fn interaction_create(&self, ctx: Context, interaction: Interaction) {
        let Interaction::Command(cmd) = interaction else { return };
        let Some(guild) = cmd.guild_id else {
            return reply(&ctx, &cmd, "Esto funciona solo en un servidor.").await;
        };
        match cmd.data.name.as_str() {
            "play" => self.play(&ctx, &cmd, guild).await,
            "skip" => self.skip(&ctx, &cmd, guild).await,
            "pause" => self.pause(&ctx, &cmd, guild, true).await,
            "resume" => self.pause(&ctx, &cmd, guild, false).await,
            "stop" => self.stop(&ctx, &cmd, guild).await,
            "queue" => self.queue(&ctx, &cmd, guild).await,
            "nowplaying" => self.now_playing(&ctx, &cmd, guild).await,
            "leave" => {
                leave(&self.bot, guild).await;
                reply(&ctx, &cmd, "Chau 👋").await;
            }
            _ => {}
        }
    }

    async fn voice_state_update(&self, ctx: Context, _old: Option<VoiceState>, new: VoiceState) {
        let Some(guild) = new.guild_id else { return };
        let me = ctx.cache.current_user().id;
        // Lo sacaron del canal a mano: se olvida de la cola.
        if new.user_id == me && new.channel_id.is_none() {
            leave(&self.bot, guild).await;
            return;
        }
        if alone(&ctx, &self.bot, guild).await {
            let bot = self.bot.clone();
            tokio::spawn(async move {
                tokio::time::sleep(ALONE_LEAVE).await;
                if alone(&ctx, &bot, guild).await {
                    info!("me voy de {guild}: quedé solo");
                    let text = bot.players.lock().await.get(&guild).map(|p| p.text);
                    leave(&bot, guild).await;
                    if let Some(text) = text {
                        say(&bot, text, "Me quedé solo en el canal, me voy.".into()).await;
                    }
                }
            });
        }
    }
}

/// Si esta en un canal de voz sin nadie mas (los bots no cuentan).
async fn alone(ctx: &Context, bot: &Bot, guild: GuildId) -> bool {
    let Some(call) = bot.voice.get(guild) else { return false };
    let Some(mine) = call.lock().await.current_channel() else { return false };
    let me = ctx.cache.current_user().id;
    let Some(g) = ctx.cache.guild(guild) else { return false };
    !g.voice_states.values().any(|v| {
        v.channel_id.map(|c| c.get()) == Some(mine.0.get())
            && v.user_id != me
            && !v.member.as_ref().is_some_and(|m| m.user.bot)
    })
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "warn,haze_bot=info".into()),
        )
        .init();
    let token = std::env::var("DISCORD_TOKEN").expect("Falta DISCORD_TOKEN (ver .env.example)");

    let http = Arc::new(Http::new(&token));
    let voice = Songbird::serenity();
    let bot = Arc::new(Bot {
        http: http.clone(),
        voice: voice.clone(),
        web: reqwest::Client::new(),
        players: Mutex::new(HashMap::new()),
    });
    tokio::spawn(keep_ytdlp_updated());

    let intents = GatewayIntents::GUILDS | GatewayIntents::GUILD_VOICE_STATES;
    let mut client = ClientBuilder::new(&token, intents)
        .event_handler(Handler { bot })
        .register_songbird_with(voice)
        .await
        .expect("no pude armar el cliente de Discord");

    // `docker stop` manda SIGTERM: se desconecta prolijo.
    let shards = client.shard_manager.clone();
    tokio::spawn(async move {
        let mut term = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()).unwrap();
        tokio::select! {
            _ = term.recv() => {}
            _ = tokio::signal::ctrl_c() => {}
        }
        info!("apagando");
        shards.shutdown_all().await;
    });

    if let Err(e) = client.start().await {
        error!("se cayo el cliente: {e}");
        std::process::exit(1);
    }
}
