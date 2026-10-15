//! Nucleo de Haze, compartido entre escritorio y Android: la cola, los ajustes y la
//! logica del reproductor. No sabe nada de Tauri ni de como suena el audio: eso lo pone
//! cada app implementando [`player::Engine`] y [`player::Listener`].

pub mod player;
pub mod queue;
pub mod settings;
pub mod song;
