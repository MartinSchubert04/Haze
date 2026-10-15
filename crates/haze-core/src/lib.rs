//! Nucleo de Haze, compartido entre escritorio y Android: la cola, los ajustes y la
//! logica del reproductor. No sabe nada de Tauri ni de como suena el audio: eso lo pone
//! cada app implementando [`player::Engine`] y [`player::Listener`]. Con la feature
//! `mqtt`, tambien el puente con Home Assistant.

#[cfg(feature = "mqtt")]
pub mod mqtt;
pub mod player;
pub mod queue;
pub mod settings;
pub mod song;
