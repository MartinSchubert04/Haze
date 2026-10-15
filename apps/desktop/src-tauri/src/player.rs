//! El reproductor vive en `haze-core`. Aca queda lo que es solo de escritorio: como le
//! llega el estado a las ventanas. El motor (mpv) esta en `mpv.rs`.

use haze_core::player::Listener;
use serde_json::json;
use tauri::{AppHandle, Emitter};

pub use haze_core::player::{Player, PlayerState, QueueView, SharedPlayer};

/// Manda el estado y la posicion a todas las ventanas abiertas.
pub struct Windows(pub AppHandle);

impl Listener for Windows {
    fn state(&self, state: &PlayerState) {
        let _ = self.0.emit("player://state", state);
    }

    fn position(&self, position: f64, duration: f64) {
        let _ = self.0.emit("player://position", json!({ "position": position, "duration": duration }));
    }
}
