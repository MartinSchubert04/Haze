// Lo que manda el backend de Rust (haze-core). Igual en escritorio y en Android.

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
    background: "glass" | "haze" | "image" | "color";
    backgroundColor: string;
    backgroundImage: string | null;
    dim: number;
    blur: number;
    tintFromArtwork: boolean;
    folderColor: string;
  };
  playback: { volume: number; shuffle: boolean; repeat: string; resume: boolean };
  behavior: {
    closeToTray: boolean;
    startHidden: boolean;
    miniOnTop: boolean;
    miniPosition: [number, number] | null;
    mainWindow: { x: number; y: number; width: number; height: number; maximized: boolean } | null;
  };
  ha: { enabled: boolean; host: string; port: number; username: string; password: string; deviceName: string };
  shortcuts: { toggle: string; next: string; previous: string; volumeUp: string; volumeDown: string; mini: string };
  songSort: string;
  folderSort: string;
};

/** Una descarga de yt-dlp. */
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

/** Lo que hay detras de un link pegado: un video o una playlist. */
export type LinkInfo = { title: string; isPlaylist: boolean; count: number; thumb: string | null; songs: Song[] };
