// Temas y paletas (los mismos que Ethereon). Los colores de cada tema estan en
// themes.css y las paletas en scripts/palettes.py, que genera palettes.css y
// palettes.gen.ts.
import { PALETTES } from "./palettes.gen";

export const THEMES = [
  { id: "default", name: "Default" },
  { id: "warp", name: "Warp" },
  { id: "poolsuite", name: "Poolsuite FM" },
  { id: "haze", name: "Haze" },
  { id: "athanor", name: "Athanor" },
  { id: "neovim", name: "Neovim" },
] as const;

export type ThemeId = (typeof THEMES)[number]["id"];
export type Palette = { id: string; name: string; swatch: string[] };

/** Las paletas de un tema; la primera es la base del tema. */
export function palettesOf(theme: string): Palette[] {
  return PALETTES[theme] ?? [];
}

/** El tema guardado, o Haze si no existe (ajustes viejos o a mano). */
export function themeOf(theme: string | undefined): ThemeId {
  return (THEMES.find((t) => t.id === theme)?.id ?? "haze") as ThemeId;
}

/** La paleta guardada, o la primera del tema si no es de ese tema. */
export function paletteOf(theme: string, palette: string | undefined): string {
  const list = palettesOf(theme);
  return list.find((p) => p.id === palette)?.id ?? list[0]?.id ?? "";
}

/** Pone el tema y la paleta en <html>: de ahi salen todas las variables de color. */
export function applyTheme(theme: string | undefined, palette: string | undefined) {
  const t = themeOf(theme);
  const el = document.documentElement;
  el.dataset.theme = t;
  el.dataset.palette = paletteOf(t, palette);
}
