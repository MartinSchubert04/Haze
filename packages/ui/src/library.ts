// El mismo tema puede estar en varias carpetas (hardlinks en la PC, copias de Syncthing en
// el telefono). Rust le pone a cada cancion la clave de su archivo (`file`); igual que
// `haze_core::song::unique_files`.

import type { Song } from "./types";

/** Que archivo es. Sin clave (streams, un cache viejo), cada ruta cuenta aparte. */
export function fileKey(s: Song): number {
  return s.file || s.id;
}

export function sameFile(a: Song | null | undefined, b: Song | null | undefined): boolean {
  return !!a && !!b && fileKey(a) === fileKey(b);
}

const depth = (s: Song) => s.folder.split("/").length;

/**
 * Una entrada por archivo, en el orden en que aparecen. De las copias queda la de la
 * carpeta mas especifica ("Categorias/Rock Nacional" antes que "All").
 */
export function uniqueFiles(songs: Song[]): Song[] {
  const out: Song[] = [];
  const at = new Map<number, number>();
  for (const s of songs) {
    const i = at.get(fileKey(s));
    if (i === undefined) {
      at.set(fileKey(s), out.length);
      out.push(s);
    } else {
      const o = out[i];
      if (depth(s) > depth(o) || (depth(s) === depth(o) && s.path < o.path)) out[i] = s;
    }
  }
  return out;
}
