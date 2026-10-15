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
pub mod youtube;

/// Si `candidate` es una version mas nueva que `current` ("1.2.10" > "1.2.9"). Lo que no
/// se entiende cuenta como 0.
pub fn newer_version(candidate: &str, current: &str) -> bool {
    let parse = |v: &str| -> Vec<u64> { v.trim_start_matches('v').split('.').map(|p| p.parse().unwrap_or(0)).collect() };
    parse(candidate) > parse(current)
}

#[cfg(test)]
mod tests {
    use super::newer_version;

    #[test]
    fn versions() {
        assert!(newer_version("0.2.0", "0.1.9"));
        assert!(newer_version("1.2.10", "1.2.9"));
        assert!(newer_version("v1.0.0", "0.9.0"));
        assert!(!newer_version("0.1.0", "0.1.0"));
        assert!(!newer_version("0.0.9", "0.1.0"));
    }
}
