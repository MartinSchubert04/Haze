// Llamadas al backend de Rust (apps/mobile/src-tauri). Los tipos son los de escritorio.
import { invoke, convertFileSrc } from "@tauri-apps/api/core";
import type { DownloadJob, LinkInfo, PlayerState, QueueEntry, QueueView, Settings, Song } from "@haze/ui/types";

export type { DownloadJob, LinkInfo, PlayerState, QueueEntry, QueueView, Settings, Song };

export type UpdateInfo = { version: string; status: "downloading" | "ready"; progress: number };

export type Bootstrap = {
  settings: Settings;
  songs: Song[];
  state: PlayerState;
  position: number;
  /** Hay permiso para leer la musica. */
  access: boolean;
  scanning: boolean;
  /** El motor ya esta conectado. */
  ready: boolean;
  version: string;
  /** Home Assistant: "off", "connected", "error" o "network" (falta el permiso de red local). */
  haStatus: string;
  downloads: DownloadJob[];
  update: UpdateInfo | null;
};

export type Control =
  | { type: "toggle" }
  | { type: "play" }
  | { type: "pause" }
  | { type: "next" }
  | { type: "previous" }
  | { type: "seek"; value: number }
  | { type: "shuffle"; value: boolean }
  | { type: "repeat"; value: string };

export const api = {
  bootstrap: () => invoke<Bootstrap>("bootstrap"),
  requestAccess: () => invoke<boolean>("request_access"),
  rescan: () => invoke("rescan"),
  playIds: (ids: number[], start: number, context: string) => invoke("play_ids", { ids, start, context }),
  /** Para listas con canciones de YouTube, que no estan en la biblioteca. */
  playSongs: (songs: Song[], start: number, context: string) => invoke("play_songs", { songs, start, context }),
  control: (control: Control) => invoke("player_control", { control }),
  enqueue: (song: Song, next: boolean) => invoke("enqueue", { song, next }),
  /** Suena ya sin reemplazar la cola: despues sigue lo que venia. */
  playNow: (song: Song) => invoke("play_now", { song }),
  queue: () => invoke<QueueView>("queue_get"),
  queueAction: (action: "jump" | "remove" | "move" | "clear", index = 0, to?: number) =>
    invoke("queue_action", { action, index, to }),
  saveSettings: (settings: Settings) => invoke("settings_set", { settings }),
  /** Android 17 pide permiso para hablar con la red local (el broker de HA). */
  requestLocalNetwork: () => invoke<boolean>("request_local_network"),
  ytSearch: (query: string) => invoke<Song[]>("yt_search", { query }),
  ytInspect: (url: string) => invoke<LinkInfo>("yt_inspect", { url }),
  ytDownload: (url: string, title: string, thumb: string | null, playlist: boolean) =>
    invoke<number>("yt_download", { url, title, thumb, playlist }),
  clearDownloads: () => invoke("downloads_clear"),
  /** Manda la app al fondo sin cerrarla (sigue sonando). */
  minimize: () => invoke("minimize"),
  /** Devuelve la version nueva, si hay (y la empieza a bajar). */
  updateCheck: () => invoke<string | null>("update_check"),
  /** `false` si antes hay que dar el permiso de instalar apps (se abre ese ajuste). */
  updateInstall: () => invoke<boolean>("update_install"),
};

/** URL de la tapa: la miniatura remota o la del archivo (la sirve Rust desde MediaStore). */
export function coverUrl(song: Song | null | undefined): string | null {
  if (!song) return null;
  if (song.thumb) return song.thumb;
  if (song.hasCover) return convertFileSrc(String(song.id), "cover");
  return null;
}
