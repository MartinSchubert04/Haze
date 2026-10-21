//! Lo que no depende de como se corre yt-dlp: leer su JSON y los argumentos para bajar
//! como MP3 con titulo, artista y tapa. Escritorio lo corre como programa; Android con
//! `youtubedl-android`.

use serde::Serialize;
use serde_json::Value;

use crate::song::{id_for, Song, SongKind};

/// Argumentos para listar sin bajar (`-J`, sin entrar a cada video de una playlist).
pub const JSON_ARGS: &[&str] = &["--ignore-config", "--no-warnings", "-J", "--flat-playlist"];

/// Busqueda en YouTube.
pub fn search_query(query: &str) -> String {
    format!("ytsearch25:{query}")
}

/// Bajar como MP3 con la tapa recortada cuadrada y el titulo limpio. Falta lo de cada app:
/// carpeta (`-P`), nombre (`-o`), playlist o no y la URL.
pub const DOWNLOAD_ARGS: &[&str] = &[
    "--ignore-config",
    "--no-warnings",
    "--newline",
    "-x",
    "--audio-format",
    "mp3",
    "--audio-quality",
    "0",
    "--embed-metadata",
    "--embed-thumbnail",
    "--convert-thumbnails",
    "jpg",
    // Tapas cuadradas, como las de un disco: se recorta el centro de la miniatura.
    "--ppa",
    r#"ThumbnailsConvertor+FFmpeg_o:-c:v mjpeg -qmin 1 -qscale:v 2 -vf crop="'if(gt(ih,iw),iw,ih)':'if(gt(iw,ih),ih,iw)'""#,
    "--replace-in-metadata",
    "uploader,channel,artist",
    " - Topic$",
    "",
    // "Artista - Tema (Official Video)" -> titulo "Tema", artista "Artista".
    "--parse-metadata",
    "title:(?P<artist>.+?) - (?P<title>.+)",
    "--replace-in-metadata",
    "title",
    r"\s*[\(\[][^\)\]]*\b(?i:official|video|audio|lyrics?|letra|visuali[sz]er|hd|hq|4k|remaster(?:ed)?)\b[^\)\]]*[\)\]]",
    "",
    "--progress-template",
    "download:DL %(progress._percent_str)s",
    "--print",
    "after_move:FILE %(filepath)s",
];

/// Sin ffmpeg no se puede convertir ni meter tags: se baja el audio tal cual lo da
/// YouTube (m4a casi siempre), con el nombre "Artista - Tema" que arma [`output_template`].
pub fn download_args_without_ffmpeg() -> Vec<&'static str> {
    let mut out = vec!["--ignore-config", "--no-warnings", "--newline", "-f", "bestaudio[ext=m4a]/bestaudio"];
    // Lo que limpia titulo y artista sirve igual para el nombre del archivo.
    let i = DOWNLOAD_ARGS.iter().position(|a| *a == "--replace-in-metadata").unwrap();
    out.extend_from_slice(&DOWNLOAD_ARGS[i..]);
    out
}

/// yt-dlp no pudo usar ffmpeg (no esta, o no carga en este telefono).
pub fn missing_ffmpeg(error: &str) -> bool {
    let e = error.to_lowercase();
    e.contains("ffmpeg") && e.contains("not found")
}

/// Nombre del archivo bajado, sin y con playlist.
pub fn output_template(playlist: bool) -> &'static str {
    if playlist {
        "%(playlist_title)s/%(artist,uploader)s - %(title)s.%(ext)s"
    } else {
        "%(artist,uploader)s - %(title)s.%(ext)s"
    }
}

/// Lo que dice yt-dlp mientras baja.
#[derive(Debug, PartialEq)]
pub enum Progress {
    /// De 0 a 1.
    Downloading(f32),
    Converting,
    /// "3 de 12" en playlists.
    Item(String),
    /// Un archivo terminado.
    File(String),
}

pub fn parse_progress(line: &str) -> Option<Progress> {
    let line = line.trim();
    if let Some(rest) = line.strip_prefix("DL ") {
        rest.trim().trim_end_matches('%').trim().parse::<f32>().ok().map(|p| Progress::Downloading(p / 100.0))
    } else if line.contains("[ExtractAudio]") || line.contains("[EmbedThumbnail]") {
        Some(Progress::Converting)
    } else if let Some(rest) = line.strip_prefix("[download] Downloading item ") {
        Some(Progress::Item(rest.replace(" of ", " de ")))
    } else {
        line.strip_prefix("FILE ").map(|p| Progress::File(p.to_string()))
    }
}

/// El mensaje de error de yt-dlp, sin el "ERROR: ".
pub fn error_line(stderr: &str) -> Option<String> {
    stderr.lines().rev().find(|l| l.contains("ERROR")).map(|l| l.replace("ERROR: ", ""))
}

fn thumb_for(id: &str) -> String {
    format!("https://i.ytimg.com/vi/{id}/mqdefault.jpg")
}

/// Una entrada del JSON de yt-dlp como cancion para escuchar sin bajar.
pub fn stream_song(v: &Value) -> Option<Song> {
    let id = v["id"].as_str()?;
    let url = v["webpage_url"]
        .as_str()
        .or_else(|| v["url"].as_str().filter(|u| u.starts_with("http")))
        .map(str::to_string)
        .unwrap_or_else(|| format!("https://www.youtube.com/watch?v={id}"));
    let is_youtube = url.contains("youtube.com") || url.contains("youtu.be");
    let thumb = if is_youtube {
        Some(thumb_for(id))
    } else {
        v["thumbnail"].as_str().map(str::to_string).or_else(|| {
            v["thumbnails"].as_array().and_then(|t| t.last()).and_then(|t| t["url"].as_str()).map(str::to_string)
        })
    };
    let artist = v["artist"]
        .as_str()
        .or_else(|| v["channel"].as_str())
        .or_else(|| v["uploader"].as_str())
        .unwrap_or_default()
        .trim_end_matches(" - Topic")
        .to_string();
    Some(Song {
        id: id_for(&url),
        path: url,
        title: v["title"].as_str().unwrap_or_default().to_string(),
        artist,
        album: String::new(),
        duration_ms: (v["duration"].as_f64().unwrap_or(0.0) * 1000.0) as u64,
        mtime: 0,
        folder: "YouTube".into(),
        kind: SongKind::Stream,
        thumb,
        has_cover: false,
        file: 0,
    })
}

/// Los resultados de una busqueda.
pub fn search_results(v: &Value) -> Vec<Song> {
    v["entries"].as_array().into_iter().flatten().filter_map(stream_song).collect()
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LinkInfo {
    pub title: String,
    pub is_playlist: bool,
    pub count: usize,
    pub thumb: Option<String>,
    pub songs: Vec<Song>,
}

/// Que hay detras de un link pegado: un video o una playlist.
pub fn link_info(v: &Value) -> Result<LinkInfo, String> {
    if v["_type"].as_str() == Some("playlist") {
        let songs = search_results(v);
        Ok(LinkInfo {
            title: v["title"].as_str().unwrap_or("Playlist").to_string(),
            is_playlist: true,
            count: songs.len(),
            thumb: songs.first().and_then(|s| s.thumb.clone()),
            songs,
        })
    } else {
        let song = stream_song(v).ok_or("No encontré nada para reproducir en ese link")?;
        Ok(LinkInfo { title: song.title.clone(), is_playlist: false, count: 1, thumb: song.thumb.clone(), songs: vec![song] })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn search_entries_become_streams() {
        let v = json!({ "entries": [
            { "id": "abc", "title": "Tema", "channel": "Banda - Topic", "duration": 201.5 },
            { "title": "sin id" },
        ]});
        let songs = search_results(&v);
        assert_eq!(songs.len(), 1);
        let s = &songs[0];
        assert_eq!(s.path, "https://www.youtube.com/watch?v=abc");
        assert_eq!(s.artist, "Banda");
        assert_eq!(s.duration_ms, 201_500);
        assert_eq!(s.kind, SongKind::Stream);
        assert_eq!(s.thumb.as_deref(), Some("https://i.ytimg.com/vi/abc/mqdefault.jpg"));
    }

    #[test]
    fn playlists_and_single_links() {
        let list = json!({ "_type": "playlist", "title": "Mix", "entries": [{ "id": "a" }, { "id": "b" }] });
        let info = link_info(&list).unwrap();
        assert!(info.is_playlist);
        assert_eq!(info.count, 2);
        let one = link_info(&json!({ "id": "a", "title": "Uno" })).unwrap();
        assert!(!one.is_playlist);
        assert_eq!(one.title, "Uno");
        assert!(link_info(&json!({})).is_err());
    }

    #[test]
    fn progress_lines() {
        assert_eq!(parse_progress("DL  42.5%"), Some(Progress::Downloading(0.425)));
        assert_eq!(parse_progress("[ExtractAudio] Destination: x.mp3"), Some(Progress::Converting));
        assert_eq!(parse_progress("[download] Downloading item 3 of 12"), Some(Progress::Item("3 de 12".into())));
        assert_eq!(parse_progress("FILE /a/b.mp3"), Some(Progress::File("/a/b.mp3".into())));
        assert_eq!(parse_progress("[youtube] abc: Downloading webpage"), None);
    }

    #[test]
    fn without_ffmpeg_keeps_names_but_does_not_convert() {
        let args = download_args_without_ffmpeg();
        assert!(!args.contains(&"-x") && !args.contains(&"--embed-thumbnail"));
        assert!(args.contains(&"--parse-metadata") && args.contains(&"--print"));
        assert!(missing_ffmpeg("Postprocessing: ffprobe and ffmpeg not found. Please install"));
        assert!(!missing_ffmpeg("Video unavailable"));
        assert_eq!(error_line("WARNING: x\nERROR: Video unavailable\n").as_deref(), Some("Video unavailable"));
    }
}
