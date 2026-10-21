// Atajos globales (registrados en Rust): andan desde cualquier programa, tambien con la
// ventana cerrada. Las teclas viven en los ajustes, como acelerador de Tauri.
import type { Settings } from "./api";
import { store } from "./store.svelte";

export type ShortcutKey = keyof Settings["shortcuts"];

const MOD = "Ctrl+Alt+Shift";

/** Mantener sincronizado con Shortcuts::default en src-tauri/src/settings.rs. */
export const defaultShortcuts: Settings["shortcuts"] = {
  toggle: `${MOD}+Space`,
  next: `${MOD}+ArrowRight`,
  previous: `${MOD}+ArrowLeft`,
  volumeUp: `${MOD}+ArrowUp`,
  volumeDown: `${MOD}+ArrowDown`,
  mini: `${MOD}+KeyM`,
  random: `${MOD}+KeyR`,
};

export const shortcutList: [ShortcutKey, string][] = [
  ["toggle", "Reproducir / pausar"],
  ["next", "Siguiente"],
  ["previous", "Anterior"],
  ["volumeUp", "Subir volumen"],
  ["volumeDown", "Bajar volumen"],
  ["mini", "Mini → ventana grande → de fondo"],
  ["random", "Canción al azar"],
];

const NAMES: Record<string, string> = {
  Space: "Espacio",
  ArrowRight: "→",
  ArrowLeft: "←",
  ArrowUp: "↑",
  ArrowDown: "↓",
  Super: "Win",
};

/** "Ctrl+Alt+Shift+ArrowRight" -> "Ctrl+Alt+Shift+→" */
export function prettyShortcut(acc: string): string {
  if (!acc) return "Sin atajo";
  return acc
    .split("+")
    .map((k) => NAMES[k] ?? k.replace(/^(Key|Digit|Numpad)/, ""))
    .join("+");
}

/** Arma el acelerador de una tecla apretada, o null si todavia es solo un modificador. */
export function shortcutFromEvent(e: KeyboardEvent): string | null {
  if (/^(Control|Alt|Shift|Meta|OS)(Left|Right)?$/.test(e.code)) return null;
  const mods = [e.ctrlKey && "Ctrl", e.altKey && "Alt", e.shiftKey && "Shift", e.metaKey && "Super"].filter(Boolean);
  // Sin modificadores solo se aceptan F1..F24 y las teclas multimedia: si no, se
  // robaria una tecla comun a todos los programas.
  if (!mods.length && !/^(F\d+|Media\w+|Audio\w+)$/.test(e.code)) return null;
  return [...mods, e.code].join("+");
}

/** "Siguiente" -> "Siguiente (Ctrl+Alt+Shift+→)" */
export function tip(label: string, key: ShortcutKey): string {
  const acc = store.settings?.shortcuts[key];
  return acc ? `${label} (${prettyShortcut(acc)})` : label;
}
