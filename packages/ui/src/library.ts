// El mismo tema puede estar en varias carpetas (hardlinks en la PC, copias de Syncthing en
// el telefono). Rust le pone a cada cancion la clave de su archivo (`file`); igual que
// `haze_core::song::unique_files`.

import type { List, Song, UserData } from "./types";

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

/** Claves de las canciones dadas de baja. */
export function removedSet(data: UserData | null | undefined): Set<number> {
  return new Set((data?.removed ?? []).filter((r) => r.removed).map((r) => r.file));
}

/** Sin las dadas de baja. */
export function visibleSongs(songs: Song[], removed: Set<number>): Song[] {
  return removed.size ? songs.filter((s) => !removed.has(fileKey(s))) : songs;
}

/** Las listas vivas, por nombre. */
export function visibleLists(data: UserData | null | undefined): List[] {
  return (data?.lists ?? []).filter((l) => !l.deleted).sort((a, b) => a.name.localeCompare(b.name, "es", { sensitivity: "base" }));
}

/**
 * Ordena las listas para mostrarlas. `count`: cuantas canciones tiene cada una en este
 * aparato (lo que se ve en pantalla).
 */
export function sortLists(lists: List[], sort: string, count: (l: List) => number): List[] {
  const byName = (a: List, b: List) => a.name.localeCompare(b.name, "es", { sensitivity: "base" });
  const out = [...lists];
  switch (sort) {
    case "name_desc":
      return out.sort((a, b) => byName(b, a));
    case "count_desc":
      return out.sort((a, b) => count(b) - count(a) || byName(a, b));
    case "count_asc":
      return out.sort((a, b) => count(a) - count(b) || byName(a, b));
    case "updated_desc":
      return out.sort((a, b) => b.updated - a.updated || byName(a, b));
    default:
      return out.sort(byName);
  }
}

/**
 * Las canciones de una lista, en su orden, con las que hay en este aparato (`songs` ya sin
 * bajas). Las que todavia no llegaron no aparecen.
 */
export function listSongs(list: List, songs: Song[]): Song[] {
  const byKey = new Map<number, Song>();
  for (const s of uniqueFiles(songs)) byKey.set(fileKey(s), s);
  const out: Song[] = [];
  for (const k of list.songs) {
    const s = byKey.get(k);
    if (s) out.push(s);
  }
  return out;
}
