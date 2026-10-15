//! Programas externos que usa la app: mpv (reproduce), yt-dlp (YouTube), ffmpeg (pasa a
//! MP3) y deno (yt-dlp lo necesita para descifrar YouTube).
//!
//! No vienen en el instalador, para que quede chico: se buscan primero en la carpeta de
//! la app y en el PATH, y si no estan se bajan la primera vez que hacen falta.

use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

static BIN_DIR: OnceLock<PathBuf> = OnceLock::new();

pub fn init(bin_dir: PathBuf) {
    let _ = std::fs::create_dir_all(&bin_dir);
    let _ = BIN_DIR.set(bin_dir);
}

pub fn bin_dir() -> &'static Path {
    BIN_DIR.get().expect("tools::init no se llamo")
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tool {
    Mpv,
    YtDlp,
    Ffmpeg,
    Deno,
}

impl Tool {
    pub fn exe(self) -> &'static str {
        match self {
            Tool::Mpv => "mpv.exe",
            Tool::YtDlp => "yt-dlp.exe",
            Tool::Ffmpeg => "ffmpeg.exe",
            Tool::Deno => "deno.exe",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Tool::Mpv => "mpv",
            Tool::YtDlp => "yt-dlp",
            Tool::Ffmpeg => "ffmpeg",
            Tool::Deno => "deno",
        }
    }
}

/// Ruta del programa si ya esta disponible.
pub fn find(tool: Tool) -> Option<PathBuf> {
    let local = bin_dir().join(tool.exe());
    if local.is_file() {
        return Some(local);
    }
    let nested = bin_dir().join("mpv").join(tool.exe());
    if nested.is_file() {
        return Some(nested);
    }
    std::env::var_os("PATH").and_then(|paths| {
        std::env::split_paths(&paths).map(|d| d.join(tool.exe())).find(|p| p.is_file())
    })
}

/// PATH para los procesos hijos: la carpeta de la app primero, asi mpv encuentra a
/// yt-dlp y yt-dlp encuentra a deno y ffmpeg.
pub fn child_path() -> std::ffi::OsString {
    let mut dirs = vec![bin_dir().to_path_buf(), bin_dir().join("mpv")];
    if let Some(p) = std::env::var_os("PATH") {
        dirs.extend(std::env::split_paths(&p));
    }
    std::env::join_paths(dirs).unwrap_or_default()
}

/// Un comando sin ventana de consola.
pub fn command(program: &Path) -> Command {
    let mut c = Command::new(program);
    c.env("PATH", child_path());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        c.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    c
}

pub fn async_command(program: &Path) -> tokio::process::Command {
    let mut c = tokio::process::Command::new(program);
    c.env("PATH", child_path());
    #[cfg(windows)]
    c.creation_flags(0x0800_0000);
    c
}

const UA: &str = "Reproductor-desktop";

fn agent() -> ureq::Agent {
    ureq::Agent::config_builder().user_agent(UA).build().into()
}

/// Baja `url` a `dest` informando el progreso (0..1).
fn download(url: &str, dest: &Path, progress: &dyn Fn(f32)) -> Result<(), String> {
    let resp = agent().get(url).call().map_err(|e| format!("No se pudo bajar {url}: {e}"))?;
    let total: u64 = resp
        .headers()
        .get("content-length")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);
    let mut reader = resp.into_body().into_reader();
    let tmp = dest.with_extension("part");
    let mut file = std::fs::File::create(&tmp).map_err(|e| e.to_string())?;
    let mut buf = vec![0u8; 64 * 1024];
    let mut done: u64 = 0;
    let mut last = 0.0f32;
    loop {
        let n = reader.read(&mut buf).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        file.write_all(&buf[..n]).map_err(|e| e.to_string())?;
        done += n as u64;
        if total > 0 {
            let p = done as f32 / total as f32;
            if p - last >= 0.01 {
                last = p;
                progress(p);
            }
        }
    }
    drop(file);
    std::fs::rename(&tmp, dest).map_err(|e| e.to_string())?;
    progress(1.0);
    Ok(())
}

fn latest_asset(repo: &str, pick: impl Fn(&str) -> bool) -> Result<String, String> {
    let url = format!("https://api.github.com/repos/{repo}/releases/latest");
    let body = agent()
        .get(&url)
        .header("Accept", "application/vnd.github+json")
        .call()
        .map_err(|e| format!("GitHub no respondio: {e}"))?
        .into_body()
        .read_to_string()
        .map_err(|e| e.to_string())?;
    let json: serde_json::Value = serde_json::from_str(&body).map_err(|e| e.to_string())?;
    json["assets"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|a| Some((a["name"].as_str()?, a["browser_download_url"].as_str()?)))
        .find(|(name, _)| pick(name))
        .map(|(_, u)| u.to_string())
        .ok_or_else(|| format!("No encontre el archivo para bajar en {repo}"))
}

/// Saca de un zip solo los archivos con esos nombres, a la carpeta bin.
fn extract_zip(zip_path: &Path, names: &[&str]) -> Result<(), String> {
    let file = std::fs::File::open(zip_path).map_err(|e| e.to_string())?;
    let mut archive = zip::ZipArchive::new(file).map_err(|e| e.to_string())?;
    for i in 0..archive.len() {
        let mut entry = archive.by_index(i).map_err(|e| e.to_string())?;
        let Some(name) = entry.enclosed_name().and_then(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()))
        else {
            continue;
        };
        if names.iter().any(|n| n.eq_ignore_ascii_case(&name)) {
            let mut out = std::fs::File::create(bin_dir().join(&name)).map_err(|e| e.to_string())?;
            std::io::copy(&mut entry, &mut out).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

/// Se asegura de que el programa este. Si hay que bajarlo, informa el progreso.
pub fn ensure(tool: Tool, progress: &dyn Fn(f32)) -> Result<PathBuf, String> {
    if let Some(p) = find(tool) {
        return Ok(p);
    }
    let bin = bin_dir();
    match tool {
        Tool::YtDlp => {
            download(
                "https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp.exe",
                &bin.join("yt-dlp.exe"),
                progress,
            )?;
        }
        Tool::Mpv => {
            // Build generico (no "v3", que exige AVX2) y sin los archivos de desarrollo.
            let url = latest_asset("shinchiro/mpv-winbuild-cmake", |n| {
                n.starts_with("mpv-x86_64-") && n.ends_with(".7z") && !n.contains("-v3-") && !n.contains("dev")
            })?;
            let archive = bin.join("mpv.7z");
            download(&url, &archive, progress)?;
            let dest = bin.join("mpv");
            let _ = std::fs::create_dir_all(&dest);
            sevenz_rust2::decompress_file(&archive, &dest).map_err(|e| format!("No se pudo descomprimir mpv: {e}"))?;
            let _ = std::fs::remove_file(&archive);
        }
        Tool::Ffmpeg => {
            let archive = bin.join("ffmpeg.zip");
            download(
                "https://github.com/yt-dlp/FFmpeg-Builds/releases/download/latest/ffmpeg-master-latest-win64-gpl.zip",
                &archive,
                progress,
            )?;
            extract_zip(&archive, &["ffmpeg.exe", "ffprobe.exe"])?;
            let _ = std::fs::remove_file(&archive);
        }
        Tool::Deno => {
            let archive = bin.join("deno.zip");
            download(
                "https://github.com/denoland/deno/releases/latest/download/deno-x86_64-pc-windows-msvc.zip",
                &archive,
                progress,
            )?;
            extract_zip(&archive, &["deno.exe"])?;
            let _ = std::fs::remove_file(&archive);
        }
    }
    find(tool).ok_or_else(|| format!("{} no quedo instalado", tool.label()))
}

/// yt-dlp tiene que estar al dia: YouTube cambia seguido y las versiones viejas dejan
/// de andar. Solo se autoactualiza la copia propia (la del PATH es del usuario).
pub fn update_ytdlp() {
    let own = bin_dir().join("yt-dlp.exe");
    if own.is_file() {
        let _ = command(&own).arg("-U").output();
    }
}
