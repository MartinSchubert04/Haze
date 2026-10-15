// Atajos globales (registrados en Rust): andan desde cualquier programa, tambien con la
// ventana cerrada. Mantener sincronizado con register_shortcuts en src-tauri/src/lib.rs.
const MOD = "Ctrl+Alt+Shift";

export const shortcuts = {
  toggle: `${MOD}+Espacio`,
  next: `${MOD}+→`,
  previous: `${MOD}+←`,
  volumeUp: `${MOD}+↑`,
  volumeDown: `${MOD}+↓`,
  mini: `${MOD}+M`,
};

export const shortcutList: [string, string][] = [
  ["Reproducir / pausar", shortcuts.toggle],
  ["Siguiente", shortcuts.next],
  ["Anterior", shortcuts.previous],
  ["Subir volumen", shortcuts.volumeUp],
  ["Bajar volumen", shortcuts.volumeDown],
  ["Abrir / cerrar el mini reproductor", shortcuts.mini],
];

/** "Siguiente" -> "Siguiente (Ctrl+Alt+Shift+→)" */
export const tip = (label: string, key: keyof typeof shortcuts) => `${label} (${shortcuts[key]})`;
