// Tipos y llamadas al backend de Rust.
import { invoke, convertFileSrc } from "@tauri-apps/api/core";

export type Song = {
  id: number;
  path: string;
  title: string;
  artist: string;
  album: string;
  durationMs: number;
  mtime: number;
  folder: string;
  kind: "local" | "stream";
  thumb: string | null;
  hasCover: boolean;
};

export type PlayerState = {
  song: Song | null;
  playing: boolean;
  buffering: boolean;
  duration: number;
  volume: number;
  shuffle: boolean;
  repeat: "off" | "all" | "one";
  context: string;
  queueVersion: number;
  position: number;
  error: string | null;
};

export type QueueEntry = { uid: number; song: Song; ephemeral: boolean };
export type QueueView = { entries: QueueEntry[]; currentIndex: number | null; context: string };

export type Settings = {
  sources: string[];
  downloadFolders: string[];
  defaultDownloadFolder: string | null;
  appearance: {
    accent: string;
    background: "glass" | "kiosk" | "image" | "color";
    backgroundColor: string;
    backgroundImage: string | null;
    dim: number;
    blur: number;
    tintFromArtwork: boolean;
    folderColor: string;
  };
  playback: { volume: number; shuffle: boolean; repeat: string; resume: boolean };
  behavior: { closeToTray: boolean; startHidden: boolean; miniOnTop: boolean; miniPosition: [number, number] | null };
  ha: { enabled: boolean; host: string; port: number; username: string; password: string; deviceName: string };
  songSort: string;
  folderSort: string;
};

export type DownloadJob = {
  id: number;
  url: string;
  title: string;
  thumb: string | null;
  folder: string;
  playlist: boolean;
  status: "queued" | "downloading" | "converting" | "done" | "error";
  progress: number;
  item: string | null;
  error: string | null;
  files: string[];
};

export type LinkInfo = { title: string; isPlaylist: boolean; count: number; thumb: string | null; songs: Song[] };

export type Bootstrap = {
  settings: Settings;
  songs: Song[];
  state: PlayerState;
  position: number;
  engine: boolean;
  scanning: boolean;
  haStatus: string;
  downloads: DownloadJob[];
  version: string;
};

export type Control =
  | { type: "toggle" }
  | { type: "play" }
  | { type: "pause" }
  | { type: "next" }
  | { type: "previous" }
  | { type: "seek"; value: number }
  | { type: "volume"; value: number }
  | { type: "shuffle"; value: boolean }
  | { type: "repeat"; value: string }
  | { type: "playFolder"; value: string };

export const api = {
  bootstrap: () => invoke<Bootstrap>("bootstrap"),
  rescan: () => invoke("rescan"),
  playIds: (ids: number[], start: number, context: string) => invoke("play_ids", { ids, start, context }),
  playSongs: (songs: Song[], start: number, context: string) => invoke("play_songs", { songs, start, context }),
  control: (control: Control) => invoke("player_control", { control }),
  enqueue: (song: Song, next: boolean) => invoke("enqueue", { song, next }),
  queue: () => invoke<QueueView>("queue_get"),
  queueAction: (action: "jump" | "remove" | "move" | "clear", index = 0, to?: number) =>
    invoke("queue_action", { action, index, to }),
  saveSettings: (settings: Settings) => invoke("settings_set", { settings }),
  ytSearch: (query: string) => invoke<Song[]>("yt_search", { query }),
  ytInspect: (url: string) => invoke<LinkInfo>("yt_inspect", { url }),
  ytPrepare: () => invoke("yt_prepare"),
  ytDownload: (url: string, title: string, thumb: string | null, folder: string, playlist: boolean) =>
    invoke<number>("yt_download", { url, title, thumb, folder, playlist }),
  clearDownloads: () => invoke("downloads_clear"),
  openMini: () => invoke("mini_open"),
  showMain: () => invoke("main_show"),
  quit: () => invoke("quit"),
  miniMoved: (x: number, y: number) => invoke("mini_moved", { x, y }),
  checkUpdate: () => invoke<{ version: string; url: string } | null>("check_update"),
};

/** URL de la tapa: la miniatura remota o la embebida (servida por Rust). */
export function coverUrl(song: Song | null | undefined): string | null {
  if (!song) return null;
  if (song.thumb) return song.thumb;
  if (song.hasCover) return convertFileSrc(String(song.id), "cover");
  return null;
}

export function fileUrl(path: string): string {
  return convertFileSrc(path);
}

export function isUrl(text: string): boolean {
  return /^https?:\/\/\S+$/i.test(text.trim());
}
