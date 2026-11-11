//! La biblioteca para la tele (apps/tizen, docs/tizen.md): la musica vive en el server
//! (`MUSIC_DIR`, montada en el contenedor) y la tele la pide por HTTP en la red de casa.
//!
//! - `GET /api/health`             que es un Haze y cuantas canciones tiene
//! - `GET /api/library`            todas las canciones (con la carpeta relativa a la musica)
//! - `GET /api/cover/{id}?s=600`   la tapa achicada a ese lado (se guarda en `DATA_DIR/covers`)
//! - `GET /api/audio/{id}`         el archivo, con Range (para adelantar)
//! - `GET /api/lyrics/{id}`        el `.lrc` que esta al lado del tema, si hay
//! - `GET /api/yt/search?q=`       buscar en YouTube
//! - `GET /api/yt/audio?url=`      escuchar de YouTube: el server pide el audio y lo pasa
//! - `POST /api/yt/save`           bajar a `MUSIC_DIR/YouTube` como MP3 (`{url}`)
//! - `GET /api/yt/saves`           como van las descargas
//! - `POST /api/rescan`            volver a leer la carpeta (tambien se hace cada 15 min)
//!
//! Para que la PC y el telefono se sincronicen (`haze_core::sync`):
//!
//! - `GET /api/sync/index`         todos los archivos de musica: ruta relativa, tamaño, fecha
//! - `GET /api/sync/file/{ruta}`   bajar uno
//! - `PUT /api/sync/file/{ruta}`   subir uno nuevo (`?mtime=`). Nunca pisa uno que ya esta
//! - `GET /api/user/{user}`        las listas y bajas de ese usuario (`haze_core::userdata`)
//! - `POST /api/user/{user}`       juntarlas con las de un aparato: devuelve lo junto
//!
//! Sin login: es solo para la red de casa (el puerto no se abre en el router).

use std::collections::{HashMap, HashSet};
use std::net::SocketAddr;
use std::path::{Path as FsPath, PathBuf};
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, Instant};

use axum::body::Body;
use axum::extract::{DefaultBodyLimit, Path, Query, Request, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use futures_util::StreamExt;
use haze_core::library::{self, normalize, Library, Song};
use haze_core::sync::{clean_user, syncable, FileEntry};
use haze_core::userdata::UserData;
use haze_core::youtube;
use tokio::io::AsyncWriteExt;
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command as Process;
use tower::ServiceExt;
use tower_http::cors::CorsLayer;
use tower_http::services::ServeFile;
use tracing::{error, info, warn};

/// Cada cuanto se vuelve a mirar la carpeta (lo que no cambio no se abre de nuevo).
const RESCAN: Duration = Duration::from_secs(15 * 60);
/// La URL directa del audio de YouTube vence a las ~6 h: se pide de nuevo antes.
const DIRECT_TTL: Duration = Duration::from_secs(60 * 60);
/// Donde se guarda lo que se baja desde la tele.
const SAVE_FOLDER: &str = "YouTube";

struct Lib {
    library: Library,
    /// Canciones con un `.lrc` al lado.
    lyrics: HashSet<u64>,
}

#[derive(Clone)]
struct Direct {
    url: String,
    headers: Vec<(String, String)>,
    /// Bytes del archivo (para Range).
    size: Option<u64>,
    mime: String,
    at: Instant,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct Save {
    id: u64,
    url: String,
    title: String,
    /// queued, downloading, converting, done, error
    status: &'static str,
    progress: f32,
    error: Option<String>,
    /// La cancion ya en la biblioteca, al terminar.
    song: Option<u64>,
}

pub struct Web {
    music: PathBuf,
    music_norm: String,
    data: PathBuf,
    lib: RwLock<Lib>,
    scanning: tokio::sync::Mutex<()>,
    direct: Mutex<HashMap<String, Direct>>,
    saves: Mutex<Vec<Save>>,
    save_tx: tokio::sync::mpsc::UnboundedSender<u64>,
    http: reqwest::Client,
    /// Uno a la vez juntando listas: dos aparatos al mismo tiempo no se pisan.
    users: tokio::sync::Mutex<()>,
}

type Shared = Arc<Web>;

/// Arranca el server en `HAZE_WEB_PORT` (7790). La musica sale de `MUSIC_DIR` (/music) y el
/// cache de `DATA_DIR` (/data).
pub async fn start() {
    let music = PathBuf::from(std::env::var("MUSIC_DIR").unwrap_or_else(|_| "/music".into()));
    let data = PathBuf::from(std::env::var("DATA_DIR").unwrap_or_else(|_| "/data".into()));
    let port: u16 = std::env::var("HAZE_WEB_PORT").ok().and_then(|p| p.parse().ok()).unwrap_or(7790);
    let bind = std::env::var("HAZE_WEB_BIND").unwrap_or_else(|_| "0.0.0.0".into());
    let _ = std::fs::create_dir_all(data.join("covers"));

    let mut library = Library::default();
    library.set(Library::load_cache(&data.join("library.json")));
    let (save_tx, save_rx) = tokio::sync::mpsc::unbounded_channel();
    let web = Arc::new(Web {
        music_norm: normalize(&music),
        music,
        data,
        lib: RwLock::new(Lib { library, lyrics: HashSet::new() }),
        scanning: tokio::sync::Mutex::new(()),
        direct: Mutex::new(HashMap::new()),
        saves: Mutex::new(Vec::new()),
        save_tx,
        http: reqwest::Client::new(),
        users: tokio::sync::Mutex::new(()),
    });
    tokio::spawn(rescan_loop(web.clone()));
    tokio::spawn(save_worker(web.clone(), save_rx));

    let app = Router::new()
        .route("/api/health", get(health))
        .route("/api/library", get(library_json))
        .route("/api/cover/{id}", get(cover))
        .route("/api/audio/{id}", get(audio))
        .route("/api/lyrics/{id}", get(lyrics))
        .route("/api/yt/search", get(yt_search))
        .route("/api/yt/audio", get(yt_audio))
        .route("/api/yt/save", post(yt_save))
        .route("/api/yt/saves", get(yt_saves))
        .route("/api/rescan", post(rescan_now))
        .route("/api/sync/index", get(sync_index))
        .route(
            "/api/sync/file/{*path}",
            get(sync_get).put(sync_put).layer(DefaultBodyLimit::max(MAX_UPLOAD as usize)),
        )
        .route("/api/user/{user}", get(user_get).post(user_post))
        // La app de la tele es una pagina local (otro origen).
        .layer(CorsLayer::permissive())
        .with_state(web);

    let addr: SocketAddr = match format!("{bind}:{port}").parse() {
        Ok(a) => a,
        Err(e) => return error!("direccion invalida {bind}:{port}: {e}"),
    };
    let listener = match tokio::net::TcpListener::bind(addr).await {
        Ok(l) => l,
        Err(e) => return error!("no pude escuchar en {addr}: {e}"),
    };
    info!("biblioteca de la tele en http://{addr}");
    if let Err(e) = axum::serve(listener, app).await {
        error!("se cayo el server de la tele: {e}");
    }
}

// ---------------------------------------------------------------------------------------
// Biblioteca

async fn rescan_loop(web: Shared) {
    loop {
        scan(&web).await;
        tokio::time::sleep(RESCAN).await;
    }
}

async fn scan(web: &Shared) {
    let Ok(_busy) = web.scanning.try_lock() else { return };
    let w = web.clone();
    let res = tokio::task::spawn_blocking(move || {
        let cached: Vec<Song> = w.lib.read().unwrap().library.songs.iter().map(|s| (**s).clone()).collect();
        let songs = library::scan(&[w.music_norm.clone()], &cached);
        let lyrics = songs.iter().filter(|s| lrc_path(s).is_file()).map(|s| s.id).collect();
        let n = songs.len();
        let mut lib = w.lib.write().unwrap();
        lib.library.set(songs);
        lib.lyrics = lyrics;
        lib.library.save_cache(&w.data.join("library.json"));
        n
    })
    .await;
    match res {
        Ok(n) => info!("biblioteca: {n} canciones"),
        Err(e) => error!("fallo el escaneo: {e}"),
    }
}

fn lrc_path(song: &Song) -> PathBuf {
    FsPath::new(&song.path).with_extension("lrc")
}

fn song(web: &Web, id: u64) -> Option<Arc<Song>> {
    web.lib.read().unwrap().library.get(id)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct TvSong<'a> {
    id: u64,
    title: &'a str,
    artist: &'a str,
    album: &'a str,
    duration_ms: u64,
    /// Relativa a la carpeta de musica ("" si esta suelta en la raiz).
    folder: &'a str,
    /// El mismo archivo en varias carpetas da lo mismo (ver `haze_core::song::file_key`).
    file: u64,
    mtime: i64,
    cover: bool,
    lyrics: bool,
}

async fn health(State(web): State<Shared>) -> Json<serde_json::Value> {
    let songs = web.lib.read().unwrap().library.songs.len();
    Json(serde_json::json!({ "ok": true, "app": "haze", "version": env!("CARGO_PKG_VERSION"), "songs": songs }))
}

async fn library_json(State(web): State<Shared>) -> Response {
    let lib = web.lib.read().unwrap();
    let songs: Vec<TvSong> = lib
        .library
        .songs
        .iter()
        .map(|s| TvSong {
            id: s.id,
            title: &s.title,
            artist: &s.artist,
            album: &s.album,
            duration_ms: s.duration_ms,
            folder: s.folder.strip_prefix(&web.music_norm).unwrap_or(&s.folder).trim_start_matches('/'),
            file: s.file_key(),
            mtime: s.mtime,
            cover: s.has_cover,
            lyrics: lib.lyrics.contains(&s.id),
        })
        .collect();
    let body = serde_json::to_vec(&serde_json::json!({ "songs": songs })).unwrap_or_default();
    ([(header::CONTENT_TYPE, "application/json"), (header::CACHE_CONTROL, "no-cache")], body).into_response()
}

#[derive(Deserialize)]
struct CoverQuery {
    s: Option<u32>,
}

/// La tapa achicada: las de los archivos llegan a 3000 px y la tele decodifica por CPU.
async fn cover(State(web): State<Shared>, Path(id): Path<u64>, Query(q): Query<CoverQuery>) -> Response {
    let Some(song) = song(&web, id) else { return StatusCode::NOT_FOUND.into_response() };
    let side = q.s.unwrap_or(600).clamp(96, 1200);
    // Por archivo: el mismo tema en varias carpetas comparte la tapa.
    let cached = web.data.join("covers").join(format!("{}-{side}.jpg", song.file_key()));
    let res = tokio::task::spawn_blocking(move || -> Option<(Vec<u8>, String)> {
        if let Ok(b) = std::fs::read(&cached) {
            return Some((b, "image/jpeg".into()));
        }
        let (bytes, mime) = library::cover(&song)?;
        match image::load_from_memory(&bytes) {
            Ok(img) => {
                let img = if img.width() > side || img.height() > side { img.thumbnail(side, side) } else { img };
                let mut out = std::io::Cursor::new(Vec::new());
                let enc = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, 86);
                if img.to_rgb8().write_with_encoder(enc).is_err() {
                    return Some((bytes, mime));
                }
                let out = out.into_inner();
                let _ = std::fs::write(&cached, &out);
                Some((out, "image/jpeg".into()))
            }
            // Un formato que no conocemos: va tal cual.
            Err(_) => Some((bytes, mime)),
        }
    })
    .await
    .ok()
    .flatten();
    match res {
        Some((bytes, mime)) => (
            [(header::CONTENT_TYPE, mime), (header::CACHE_CONTROL, "max-age=604800".into())],
            bytes,
        )
            .into_response(),
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

fn audio_mime(path: &str) -> &'static str {
    let ext = path.rsplit('.').next().unwrap_or("").to_ascii_lowercase();
    match ext.as_str() {
        "mp3" => "audio/mpeg",
        "m4a" | "mp4" | "aac" | "alac" => "audio/mp4",
        "flac" => "audio/flac",
        "ogg" | "opus" => "audio/ogg",
        "wav" => "audio/wav",
        _ => "application/octet-stream",
    }
}

async fn audio(State(web): State<Shared>, Path(id): Path<u64>, req: Request) -> Response {
    let Some(song) = song(&web, id) else { return StatusCode::NOT_FOUND.into_response() };
    let mime: mime::Mime = audio_mime(&song.path).parse().unwrap_or(mime::APPLICATION_OCTET_STREAM);
    match ServeFile::new_with_mime(&song.path, &mime).oneshot(req).await {
        Ok(r) => r.map(Body::new).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

async fn lyrics(State(web): State<Shared>, Path(id): Path<u64>) -> Response {
    let Some(song) = song(&web, id) else { return StatusCode::NOT_FOUND.into_response() };
    let Ok(bytes) = tokio::fs::read(lrc_path(&song)).await else { return StatusCode::NOT_FOUND.into_response() };
    // Algunos .lrc vienen en Latin-1.
    let text = match String::from_utf8(bytes) {
        Ok(t) => t,
        Err(e) => e.into_bytes().iter().map(|&b| b as char).collect(),
    };
    ([(header::CONTENT_TYPE, "text/plain; charset=utf-8")], text.trim_start_matches('\u{feff}').to_string()).into_response()
}

async fn rescan_now(State(web): State<Shared>) -> StatusCode {
    tokio::spawn(async move { scan(&web).await });
    StatusCode::ACCEPTED
}

// ---------------------------------------------------------------------------------------
// Sincronizar (la PC y el telefono)

/// Un tema no pesa mas que esto: lo demas es un error o algo que no es musica.
const MAX_UPLOAD: u64 = 1024 * 1024 * 1024;
/// Lo que se esta subiendo, antes de quedar en su lugar.
const UPLOAD_TMP: &str = ".haze-subiendo";

async fn sync_index(State(web): State<Shared>) -> Response {
    let root = web.music.clone();
    let files = tokio::task::spawn_blocking(move || {
        walkdir::WalkDir::new(&root)
            .into_iter()
            .filter_entry(|e| e.depth() == 0 || !e.file_name().to_string_lossy().starts_with('.'))
            .filter_map(Result::ok)
            .filter(|e| e.file_type().is_file())
            .filter_map(|e| {
                let rel = e.path().strip_prefix(&root).ok()?.to_string_lossy().replace('\\', "/");
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
            .collect::<Vec<_>>()
    })
    .await
    .unwrap_or_default();
    ([(header::CACHE_CONTROL, "no-cache")], Json(files)).into_response()
}

/// La ruta en disco de un archivo de la musica, si la ruta pedida es valida.
fn music_file(web: &Web, path: &str) -> Option<PathBuf> {
    syncable(path).then(|| web.music.join(path))
}

async fn sync_get(State(web): State<Shared>, Path(path): Path<String>, req: Request) -> Response {
    let Some(file) = music_file(&web, &path) else { return bad("ruta invalida") };
    let mime: mime::Mime = audio_mime(&path).parse().unwrap_or(mime::APPLICATION_OCTET_STREAM);
    match ServeFile::new_with_mime(&file, &mime).oneshot(req).await {
        Ok(r) => r.map(Body::new).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

#[derive(Deserialize)]
struct PutQuery {
    #[serde(default)]
    mtime: i64,
}

/// Sube un archivo nuevo. Si ya hay uno con esa ruta, 409: nunca se pisa nada.
async fn sync_put(
    State(web): State<Shared>,
    Path(path): Path<String>,
    Query(q): Query<PutQuery>,
    body: Body,
) -> Response {
    let Some(dest) = music_file(&web, &path) else { return bad("ruta invalida") };
    if tokio::fs::try_exists(&dest).await.unwrap_or(true) {
        return StatusCode::CONFLICT.into_response();
    }
    let tmp_dir = web.music.join(UPLOAD_TMP);
    if let Err(e) = tokio::fs::create_dir_all(&tmp_dir).await {
        return (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response();
    }
    let tmp = tmp_dir.join(format!("{:x}.part", haze_core::song::id_for(&format!("{path}|{:?}", Instant::now()))));
    let res: Result<(), String> = async {
        let mut out = tokio::fs::File::create(&tmp).await.map_err(|e| e.to_string())?;
        let mut stream = body.into_data_stream();
        let mut n: u64 = 0;
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|e| e.to_string())?;
            n += chunk.len() as u64;
            if n > MAX_UPLOAD {
                return Err("demasiado grande".into());
            }
            out.write_all(&chunk).await.map_err(|e| e.to_string())?;
        }
        out.flush().await.map_err(|e| e.to_string())?;
        drop(out);
        if let Some(dir) = dest.parent() {
            tokio::fs::create_dir_all(dir).await.map_err(|e| e.to_string())?;
        }
        // Pudo llegar otro mientras tanto.
        if tokio::fs::try_exists(&dest).await.unwrap_or(true) {
            return Err("ya existe".into());
        }
        tokio::fs::rename(&tmp, &dest).await.map_err(|e| e.to_string())?;
        if q.mtime > 0 {
            let d = dest.clone();
            let _ = tokio::task::spawn_blocking(move || {
                let f = std::fs::File::options().write(true).open(d)?;
                f.set_modified(std::time::UNIX_EPOCH + Duration::from_secs(q.mtime as u64))
            })
            .await;
        }
        Ok(())
    }
    .await;
    let _ = tokio::fs::remove_file(&tmp).await;
    match res {
        Ok(()) => {
            info!("sincronizar: llego {path}");
            StatusCode::CREATED.into_response()
        }
        Err(e) if e == "ya existe" => StatusCode::CONFLICT.into_response(),
        Err(e) => {
            warn!("sincronizar: no se pudo guardar {path}: {e}");
            (StatusCode::INTERNAL_SERVER_ERROR, e).into_response()
        }
    }
}

fn user_path(web: &Web, user: &str) -> PathBuf {
    web.data.join("users").join(format!("{}.json", clean_user(user)))
}

async fn user_get(State(web): State<Shared>, Path(user): Path<String>) -> Response {
    let path = user_path(&web, &user);
    let data = tokio::task::spawn_blocking(move || UserData::load(&path)).await.unwrap_or_default();
    ([(header::CACHE_CONTROL, "no-cache")], Json(data)).into_response()
}

async fn user_post(State(web): State<Shared>, Path(user): Path<String>, Json(theirs): Json<UserData>) -> Response {
    let _one = web.users.lock().await;
    let path = user_path(&web, &user);
    let res = tokio::task::spawn_blocking(move || {
        let merged = UserData::load(&path).merge(&theirs);
        merged.save(&path).map(|_| merged)
    })
    .await;
    match res {
        Ok(Ok(merged)) => Json(merged).into_response(),
        Ok(Err(e)) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

// ---------------------------------------------------------------------------------------
// YouTube

/// Solo links de YouTube: lo que se pasa a yt-dlp lo eligio alguien en la red.
fn youtube_url(url: &str) -> bool {
    ["https://www.youtube.com/", "https://youtube.com/", "https://music.youtube.com/", "https://youtu.be/", "https://m.youtube.com/"]
        .iter()
        .any(|p| url.starts_with(p))
        && !url.chars().any(char::is_whitespace)
}

fn bad(msg: &str) -> Response {
    (StatusCode::BAD_REQUEST, Json(serde_json::json!({ "error": msg }))).into_response()
}

#[derive(Deserialize)]
struct SearchQuery {
    q: String,
}

async fn yt_search(Query(q): Query<SearchQuery>) -> Response {
    let query = q.q.trim();
    if query.is_empty() {
        return Json(Vec::<Song>::new()).into_response();
    }
    let mut cmd = Process::new("yt-dlp");
    cmd.args(youtube::JSON_ARGS).arg("--").arg(youtube::search_query(query)).kill_on_drop(true);
    let out = match tokio::time::timeout(Duration::from_secs(60), cmd.output()).await {
        Ok(Ok(o)) => o,
        Ok(Err(e)) => return (StatusCode::BAD_GATEWAY, Json(serde_json::json!({ "error": e.to_string() }))).into_response(),
        Err(_) => return (StatusCode::GATEWAY_TIMEOUT, Json(serde_json::json!({ "error": "YouTube tardó demasiado" }))).into_response(),
    };
    if !out.status.success() {
        let msg = youtube::error_line(&String::from_utf8_lossy(&out.stderr)).unwrap_or_else(|| "yt-dlp falló".into());
        return (StatusCode::BAD_GATEWAY, Json(serde_json::json!({ "error": msg }))).into_response();
    }
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap_or_default();
    Json(youtube::search_results(&v)).into_response()
}

/// La URL directa del audio (m4a si hay: la tele lo reproduce seguro) y los headers que pide.
async fn resolve_direct(web: &Web, url: &str, fresh: bool) -> Result<Direct, String> {
    if !fresh {
        if let Some(d) = web.direct.lock().unwrap().get(url).filter(|d| d.at.elapsed() < DIRECT_TTL) {
            return Ok(d.clone());
        }
    }
    let mut cmd = Process::new("yt-dlp");
    // Solo formatos de un archivo por HTTPS: los HLS (m3u8) y DASH no los puede pasar tal cual.
    let format = "bestaudio[ext=m4a][protocol=https]/bestaudio[protocol=https]/best[protocol=https]";
    cmd.args(["--ignore-config", "--no-warnings", "-j", "--no-playlist", "-f", format, "--", url])
        .kill_on_drop(true);
    let out = tokio::time::timeout(Duration::from_secs(60), cmd.output())
        .await
        .map_err(|_| "YouTube tardó demasiado".to_string())?
        .map_err(|e| e.to_string())?;
    if !out.status.success() {
        return Err(youtube::error_line(&String::from_utf8_lossy(&out.stderr)).unwrap_or_else(|| "yt-dlp falló".into()));
    }
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).map_err(|e| e.to_string())?;
    let direct_url = v["url"].as_str().ok_or("yt-dlp no devolvió el audio")?.to_string();
    // El tamaño: el de yt-dlp, o el `clen` que trae la URL de YouTube.
    let size = v["filesize"].as_u64().or_else(|| {
        let q = direct_url.split_once('?')?.1;
        q.split('&').find_map(|kv| kv.strip_prefix("clen=")?.parse().ok())
    });
    let mime = match v["ext"].as_str() {
        Some("webm") => "audio/webm",
        Some("mp3") => "audio/mpeg",
        _ => "audio/mp4",
    };
    let direct = Direct {
        size,
        mime: mime.to_string(),
        url: direct_url,
        headers: v["http_headers"]
            .as_object()
            .map(|h| h.iter().filter_map(|(k, v)| Some((k.clone(), v.as_str()?.to_string()))).collect())
            .unwrap_or_default(),
        at: Instant::now(),
    };
    web.direct.lock().unwrap().insert(url.to_string(), direct.clone());
    Ok(direct)
}

#[derive(Deserialize)]
struct UrlQuery {
    url: String,
}

/// Lo que se le pide a YouTube por vez (yt-dlp usa lo mismo).
const YT_CHUNK: u64 = 10 * 1024 * 1024;

/// "bytes=100-" / "bytes=100-200" → (100, None) / (100, Some(200)).
fn parse_range(h: &HeaderMap) -> Option<(u64, Option<u64>)> {
    let v = h.get(header::RANGE)?.to_str().ok()?.trim().strip_prefix("bytes=")?;
    let (a, b) = v.split_once('-')?;
    Some((a.trim().parse().ok()?, b.trim().parse().ok()))
}

/// Pide `[pos, end]` de a pedazos con `&range=` en la URL: YouTube contesta 403 al header
/// `Range` (y a los pedidos sin rango), asi que la tele no puede ir directo.
struct Chunks {
    http: reqwest::Client,
    direct: Direct,
    pos: u64,
    end: u64,
    resp: Option<reqwest::Response>,
}

impl Chunks {
    /// Una URL recien pedida a yt-dlp da 403 durante los primeros ~3 s y despues anda: se
    /// reintenta la misma un rato antes de darla por mala.
    async fn open(&mut self) -> Result<(), reqwest::Error> {
        let e = (self.pos + YT_CHUNK - 1).min(self.end);
        let mut tries = 0;
        loop {
            let mut req = self.http.get(format!("{}&range={}-{e}", self.direct.url, self.pos));
            for (k, v) in &self.direct.headers {
                req = req.header(k, v);
            }
            let resp = req.send().await?;
            if resp.status() == reqwest::StatusCode::FORBIDDEN && tries < 8 {
                tries += 1;
                tokio::time::sleep(Duration::from_millis(750)).await;
                continue;
            }
            self.resp = Some(resp.error_for_status()?);
            return Ok(());
        }
    }
}

/// Pasa el audio de YouTube a la tele, con Range para adelantar.
async fn yt_audio(State(web): State<Shared>, Query(q): Query<UrlQuery>, headers: HeaderMap) -> Response {
    if !youtube_url(&q.url) {
        return bad("solo links de YouTube");
    }
    let range = parse_range(&headers);
    // Si la URL sigue rechazada (vencio), se pide otra; hasta dos veces.
    for attempt in 0..3 {
        let direct = match resolve_direct(&web, &q.url, attempt > 0).await {
            Ok(d) => d,
            Err(e) => return (StatusCode::BAD_GATEWAY, Json(serde_json::json!({ "error": e }))).into_response(),
        };
        let Some(total) = direct.size.filter(|&n| n > 0) else {
            return (StatusCode::BAD_GATEWAY, Json(serde_json::json!({ "error": "YouTube no dijo el tamaño del audio" })))
                .into_response();
        };
        let (start, end) = match range {
            Some((s, e)) => (s, e.unwrap_or(total - 1).min(total - 1)),
            None => (0, total - 1),
        };
        if start > end {
            return Response::builder()
                .status(StatusCode::RANGE_NOT_SATISFIABLE)
                .header(header::CONTENT_RANGE, format!("bytes */{total}"))
                .body(Body::empty())
                .unwrap_or_else(|_| StatusCode::RANGE_NOT_SATISFIABLE.into_response());
        }
        let mut chunks = Chunks { http: web.http.clone(), direct: direct.clone(), pos: start, end, resp: None };
        if let Err(e) = chunks.open().await {
            // Vencida o rechazada: se pide otra.
            if e.status() == Some(reqwest::StatusCode::FORBIDDEN) && attempt < 2 {
                warn!("YouTube rechazo la URL, pido otra");
                continue;
            }
            return (StatusCode::BAD_GATEWAY, e.to_string()).into_response();
        }
        let stream = futures_util::stream::try_unfold(chunks, |mut c| async move {
            loop {
                if let Some(r) = c.resp.as_mut() {
                    match r.chunk().await? {
                        Some(b) => {
                            c.pos += b.len() as u64;
                            return Ok::<_, reqwest::Error>(Some((b, c)));
                        }
                        None => c.resp = None,
                    }
                }
                if c.pos > c.end {
                    return Ok(None);
                }
                c.open().await?;
            }
        });
        let mut out = Response::builder()
            .status(if range.is_some() { StatusCode::PARTIAL_CONTENT } else { StatusCode::OK })
            .header(header::CONTENT_TYPE, direct.mime.as_str())
            .header(header::ACCEPT_RANGES, "bytes")
            .header(header::CONTENT_LENGTH, end - start + 1);
        if range.is_some() {
            out = out.header(header::CONTENT_RANGE, format!("bytes {start}-{end}/{total}"));
        }
        return out.body(Body::from_stream(stream)).unwrap_or_else(|_| StatusCode::BAD_GATEWAY.into_response());
    }
    StatusCode::BAD_GATEWAY.into_response()
}

#[derive(Deserialize)]
struct SaveBody {
    url: String,
    #[serde(default)]
    title: String,
}

async fn yt_save(State(web): State<Shared>, Json(b): Json<SaveBody>) -> Response {
    if !youtube_url(&b.url) {
        return bad("solo links de YouTube");
    }
    let id = {
        let mut saves = web.saves.lock().unwrap();
        if let Some(s) = saves.iter().find(|s| s.url == b.url && s.status != "error") {
            return Json(s.clone()).into_response();
        }
        let id = saves.iter().map(|s| s.id).max().unwrap_or(0) + 1;
        saves.push(Save { id, url: b.url, title: b.title, status: "queued", progress: 0.0, error: None, song: None });
        // Solo las ultimas: es para mostrar el avance, no un historial.
        let len = saves.len();
        if len > 30 {
            saves.drain(..len - 30);
        }
        id
    };
    let _ = web.save_tx.send(id);
    let s = web.saves.lock().unwrap().iter().find(|s| s.id == id).cloned();
    Json(s).into_response()
}

async fn yt_saves(State(web): State<Shared>) -> Json<Vec<Save>> {
    Json(web.saves.lock().unwrap().clone())
}

fn set_save(web: &Web, id: u64, f: impl FnOnce(&mut Save)) {
    if let Some(s) = web.saves.lock().unwrap().iter_mut().find(|s| s.id == id) {
        f(s);
    }
}

/// Las descargas van de a una, como en escritorio.
async fn save_worker(web: Shared, mut rx: tokio::sync::mpsc::UnboundedReceiver<u64>) {
    while let Some(id) = rx.recv().await {
        let Some(url) = web.saves.lock().unwrap().iter().find(|s| s.id == id).map(|s| s.url.clone()) else { continue };
        set_save(&web, id, |s| s.status = "downloading");
        match download(&web, id, &url).await {
            Ok(song) => set_save(&web, id, |s| {
                s.status = "done";
                s.progress = 1.0;
                s.song = song;
            }),
            Err(e) => {
                warn!("no se pudo bajar {url}: {e}");
                set_save(&web, id, |s| {
                    s.status = "error";
                    s.error = Some(e);
                });
            }
        }
    }
}

async fn download(web: &Shared, id: u64, url: &str) -> Result<Option<u64>, String> {
    let folder = web.music.join(SAVE_FOLDER);
    tokio::fs::create_dir_all(&folder).await.map_err(|e| e.to_string())?;
    let mut child = Process::new("yt-dlp")
        .args(youtube::DOWNLOAD_ARGS)
        .arg("--no-playlist")
        .arg("-P")
        .arg(&folder)
        .arg("-o")
        .arg(youtube::output_template(false))
        .arg("--")
        .arg(url)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .map_err(|e| format!("no pude correr yt-dlp: {e}"))?;
    let stderr = child.stderr.take();
    let err_task = tokio::spawn(async move {
        let mut text = String::new();
        if let Some(e) = stderr {
            let mut lines = BufReader::new(e).lines();
            while let Ok(Some(l)) = lines.next_line().await {
                text.push_str(&l);
                text.push('\n');
            }
        }
        text
    });
    let mut file = None;
    if let Some(out) = child.stdout.take() {
        let mut lines = BufReader::new(out).lines();
        while let Ok(Some(line)) = lines.next_line().await {
            match youtube::parse_progress(&line) {
                Some(youtube::Progress::Downloading(p)) => set_save(web, id, |s| s.progress = p),
                Some(youtube::Progress::Converting) => set_save(web, id, |s| s.status = "converting"),
                Some(youtube::Progress::File(f)) => file = Some(f),
                _ => {}
            }
        }
    }
    let status = child.wait().await.map_err(|e| e.to_string())?;
    let stderr = err_task.await.unwrap_or_default();
    if !status.success() {
        return Err(youtube::error_line(&stderr).unwrap_or_else(|| "yt-dlp falló".into()));
    }
    let Some(file) = file else { return Ok(None) };
    let path = PathBuf::from(file);
    let w = web.clone();
    let song = tokio::task::spawn_blocking(move || {
        let song = library::read_file(&path)?;
        let mut lib = w.lib.write().unwrap();
        let s = lib.library.upsert(song);
        lib.library.save_cache(&w.data.join("library.json"));
        Some(s.id)
    })
    .await
    .ok()
    .flatten();
    Ok(song)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_youtube_links() {
        assert!(youtube_url("https://www.youtube.com/watch?v=abc"));
        assert!(youtube_url("https://youtu.be/abc"));
        assert!(!youtube_url("http://192.168.0.1/x"));
        assert!(!youtube_url("--exec rm"));
        assert!(!youtube_url("https://www.youtube.com/watch?v=a b"));
    }

    #[test]
    fn ranges() {
        let mut h = HeaderMap::new();
        assert_eq!(parse_range(&h), None);
        h.insert(header::RANGE, "bytes=0-".parse().unwrap());
        assert_eq!(parse_range(&h), Some((0, None)));
        h.insert(header::RANGE, "bytes=100-199".parse().unwrap());
        assert_eq!(parse_range(&h), Some((100, Some(199))));
    }

    #[test]
    fn mimes() {
        assert_eq!(audio_mime("/m/a.MP3"), "audio/mpeg");
        assert_eq!(audio_mime("/m/a.m4a"), "audio/mp4");
    }
}
