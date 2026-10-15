//! Panel de medios de Windows (SMTC): el control que aparece al tocar las teclas de
//! volumen, las teclas multimedia del teclado y lo que leen apps como HASS.Agent.
//!
//! Se usa un `MediaPlayer` de WinRT solo por sus controles, sin reproducir nada con el:
//! asi no hace falta una ventana (la principal se destruye al cerrarla).

use std::sync::mpsc;

use tokio::sync::watch;

use crate::library::Song;
use crate::player::PlayerState;
use crate::Control;

pub fn start(mut rx: watch::Receiver<PlayerState>, control: impl Fn(Control) + Send + Sync + 'static) {
    let (tx, updates) = mpsc::channel::<PlayerState>();
    std::thread::spawn(move || {
        #[cfg(windows)]
        if let Err(e) = imp::run(updates, control) {
            log::warn!("SMTC no disponible: {e}");
        }
    });
    tauri::async_runtime::spawn(async move {
        loop {
            let s = rx.borrow_and_update().clone();
            if tx.send(s).is_err() {
                return;
            }
            if rx.changed().await.is_err() {
                return;
            }
        }
    });
}

pub fn cover_bytes(song: &Song) -> Option<Vec<u8>> {
    if let Some(url) = &song.thumb {
        let mut body = ureq::get(url).call().ok()?.into_body();
        return body.with_config().limit(5 * 1024 * 1024).read_to_vec().ok();
    }
    crate::library::cover(song).map(|(b, _)| b)
}

#[cfg(windows)]
mod imp {
    use super::*;
    use windows::core::HSTRING;
    use windows::Foundation::{TimeSpan, TypedEventHandler};
    use windows::Media::Playback::MediaPlayer;
    use windows::Media::*;
    use windows::Storage::Streams::{DataWriter, InMemoryRandomAccessStream, RandomAccessStreamReference};
    use windows::Win32::System::WinRT::{RoInitialize, RO_INIT_MULTITHREADED};

    fn span(seconds: f64) -> TimeSpan {
        TimeSpan { Duration: (seconds * 10_000_000.0) as i64 }
    }

    pub fn run(updates: mpsc::Receiver<PlayerState>, control: impl Fn(Control) + Send + Sync + 'static) -> windows::core::Result<()> {
        unsafe { RoInitialize(RO_INIT_MULTITHREADED)? };
        let player = MediaPlayer::new()?;
        player.CommandManager()?.SetIsEnabled(false)?;
        let smtc = player.SystemMediaTransportControls()?;
        smtc.SetIsEnabled(true)?;
        smtc.SetIsPlayEnabled(true)?;
        smtc.SetIsPauseEnabled(true)?;
        smtc.SetIsNextEnabled(true)?;
        smtc.SetIsPreviousEnabled(true)?;
        smtc.SetIsStopEnabled(false)?;

        smtc.ButtonPressed(&TypedEventHandler::new(
            move |_, args: windows::core::Ref<SystemMediaTransportControlsButtonPressedEventArgs>| {
                if let Some(args) = args.as_ref() {
                    let c = match args.Button()? {
                        SystemMediaTransportControlsButton::Play => Some(Control::Play),
                        SystemMediaTransportControlsButton::Pause => Some(Control::Pause),
                        SystemMediaTransportControlsButton::Next => Some(Control::Next),
                        SystemMediaTransportControlsButton::Previous => Some(Control::Previous),
                        _ => None,
                    };
                    if let Some(c) = c {
                        control(c);
                    }
                }
                Ok(())
            },
        ))?;

        let updater = smtc.DisplayUpdater()?;
        updater.SetType(MediaPlaybackType::Music)?;
        let mut last_song: Option<u64> = None;

        while let Ok(state) = updates.recv() {
            let status = match (&state.song, state.playing) {
                (None, _) => MediaPlaybackStatus::Closed,
                (Some(_), true) => MediaPlaybackStatus::Playing,
                (Some(_), false) => MediaPlaybackStatus::Paused,
            };
            let _ = smtc.SetPlaybackStatus(status);

            if let Some(song) = &state.song {
                if last_song != Some(song.id) || song.title.is_empty() {
                    last_song = Some(song.id);
                    let music = updater.MusicProperties()?;
                    music.SetTitle(&HSTRING::from(song.title.as_str()))?;
                    music.SetArtist(&HSTRING::from(song.artist.as_str()))?;
                    music.SetAlbumTitle(&HSTRING::from(song.album.as_str()))?;
                    let _ = updater.SetThumbnail(None);
                    if let Some(bytes) = cover_bytes(song) {
                        if let Ok(r) = thumbnail(&bytes) {
                            let _ = updater.SetThumbnail(&r);
                        }
                    }
                    updater.Update()?;
                }
                let tl = SystemMediaTransportControlsTimelineProperties::new()?;
                tl.SetStartTime(span(0.0))?;
                tl.SetEndTime(span(state.duration))?;
                tl.SetMinSeekTime(span(0.0))?;
                tl.SetMaxSeekTime(span(state.duration))?;
                tl.SetPosition(span(state.position))?;
                let _ = smtc.UpdateTimelineProperties(&tl);
            } else if last_song.take().is_some() {
                let _ = updater.ClearAll();
                let _ = updater.SetType(MediaPlaybackType::Music);
                let _ = updater.Update();
            }
        }
        Ok(())
    }

    fn thumbnail(bytes: &[u8]) -> windows::core::Result<RandomAccessStreamReference> {
        let stream = InMemoryRandomAccessStream::new()?;
        let writer = DataWriter::CreateDataWriter(&stream.GetOutputStreamAt(0)?)?;
        writer.WriteBytes(bytes)?;
        writer.StoreAsync()?.get()?;
        writer.FlushAsync()?.get()?;
        writer.DetachStream()?;
        RandomAccessStreamReference::CreateFromStream(&stream)
    }
}
