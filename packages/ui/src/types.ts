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
  /** Clave del archivo: la misma para el tema en varias carpetas. 0 si no se sabe. */
  file: number;
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
  /** Carpetas quitadas de la biblioteca (Android). */
  hiddenFolders: string[];
  downloadFolders: string[];
  defaultDownloadFolder: string | null;
  appearance: {
    /** Ver THEMES en @haze/ui/themes. */
    theme: string;
    /** Vacia: la primera del tema. */
    palette: string;
    /** Ya no se usa: el acento sale de la paleta. */
    accent: string;
    /** "haze" es de antes de los temas: se toma como "theme". */
    background: "theme" | "glass" | "haze" | "image" | "color";
    backgroundColor: string;
    backgroundImage: string | null;
    dim: number;
    blur: number;
    tintFromArtwork: boolean;
    /** Vacio: el acento de la paleta. */
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
  /** Bot de Discord (escritorio). */
  discord: { enabled: boolean; token: string; muteLocal: boolean };
  /** Sincronizar con el server de casa (`haze_core::sync`). `folder` es solo de escritorio. */
  sync: { enabled: boolean; server: string; user: string; folder: string };
  shortcuts: { toggle: string; next: string; previous: string; volumeUp: string; volumeDown: string; mini: string; random: string };
  songSort: string;
  folderSort: string;
  listSort: string;
  listSongSort: string;
};

/** Una lista de Haze (`haze_core::userdata`). Las canciones son claves de archivo. */
export type List = { id: string; name: string; songs: number[]; updated: number; deleted: boolean };
/** Una cancion dada de baja: no se ve en ningun aparato, pero el archivo sigue. */
export type Removed = { file: number; title: string; removed: boolean; updated: number };
export type UserData = { lists: List[]; removed: Removed[] };

/** Un cambio en las listas o las bajas (`haze_core::userdata::Op`). */
export type UserOp =
  | { type: "create"; id?: string; name: string; files?: number[] }
  | { type: "rename"; id: string; name: string }
  | { type: "delete"; id: string }
  | { type: "add"; id: string; files: number[] }
  | { type: "remove"; id: string; file: number }
  | { type: "move"; id: string; file: number; to: number }
  | { type: "setRemoved"; files: [number, string][]; removed: boolean };

/** Como fue la ultima sincronizacion. `status` vacio: todavia ninguna; "network": falta el permiso de red local (Android). */
export type SyncReport = {
  status: "" | "ok" | "away" | "error" | "running" | "off" | "network";
  at: number;
  downloaded: number;
  uploaded: number;
  failed: number;
  error: string | null;
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
