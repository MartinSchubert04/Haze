// Estado global de la interfaz. Rust es la fuente de verdad: esto es un espejo que se
// actualiza con los eventos que emite el backend.
import { listen } from "@tauri-apps/api/event";
import { api, type DownloadJob, type PlayerState, type Settings, type Song } from "./api";
import { folderName, sortKey } from "./format";

export type View =
  | { name: "songs" }
  | { name: "folders" }
  | { name: "folder"; path: string }
  | { name: "youtube"; query?: string }
  | { name: "downloads" }
  | { name: "queue" }
  | { name: "settings" };

export type Folder = { path: string; name: string; songs: Song[]; coverSong: Song | null };

const emptyState: PlayerState = {
  song: null,
  playing: false,
  buffering: false,
  duration: 0,
  volume: 70,
  shuffle: false,
  repeat: "all",
  context: "",
  queueVersion: 0,
  position: 0,
  error: null,
};

class Store {
  ready = $state(false);
  engineReady = $state(false);
  version = $state("");
  songs = $state.raw<Song[]>([]);
  settings = $state<Settings | null>(null);
  player = $state<PlayerState>(emptyState);
  scanning = $state(false);
  haStatus = $state("off");
  downloads = $state<DownloadJob[]>([]);
  toolProgress = $state<{ tool: string; progress: number } | null>(null);
  view = $state<View>({ name: "songs" });
  history = $state<View[]>([]);
  search = $state("");
  nowPlayingOpen = $state(false);
  toasts = $state<{ id: number; text: string; kind: "info" | "error" }[]>([]);
  /** Link pegado que espera que el usuario elija que hacer. */
  pendingLink = $state<string | null>(null);

  // Posicion interpolada: Rust la manda cada ~400 ms y aca se completa entre medio.
  private posBase = 0;
  private posAt = 0;
  position = $state(0);

  folders = $derived.by(() => groupFolders(this.songs));

  private toastId = 0;

  async init() {
    const b = await api.bootstrap();
    this.settings = b.settings;
    this.songs = b.songs;
    this.player = b.state;
    this.setPosition(b.position);
    this.scanning = b.scanning;
    this.haStatus = b.haStatus;
    this.downloads = b.downloads;
    this.version = b.version;
    this.engineReady = b.engine;
    this.ready = true;

    listen<PlayerState>("player://state", (e) => {
      const prevErr = this.player.error;
      this.player = e.payload;
      this.setPosition(e.payload.position);
      if (e.payload.error && e.payload.error !== prevErr) this.toast(e.payload.error, "error");
    });
    listen<{ position: number; duration: number }>("player://position", (e) => this.setPosition(e.payload.position));
    listen<Song[]>("library://changed", (e) => (this.songs = e.payload));
    listen<Song>("library://added", (e) => {
      const s = e.payload;
      this.songs = [...this.songs.filter((x) => x.id !== s.id), s];
    });
    listen<boolean>("library://scanning", (e) => (this.scanning = e.payload));
    listen<Settings>("settings://changed", (e) => (this.settings = e.payload));
    listen<string>("ha://status", (e) => (this.haStatus = e.payload));
    listen<DownloadJob[]>("downloads://changed", (e) => (this.downloads = e.payload));
    listen<string>("downloads://done", (e) => this.toast(`Listo: ${e.payload}`));
    listen<{ tool: string; progress: number }>("tools://progress", (e) => {
      this.toolProgress = e.payload.progress >= 1 ? null : e.payload;
    });
    listen<string>("app://error", (e) => this.toast(e.payload, "error"));
    listen("app://ready", () => (this.engineReady = true));

    setInterval(() => this.tick(), 250);
  }

  private setPosition(p: number) {
    this.posBase = p;
    this.posAt = performance.now();
    this.position = p;
  }

  private tick() {
    if (!this.player.playing || this.player.buffering) return;
    const p = this.posBase + (performance.now() - this.posAt) / 1000;
    this.position = this.player.duration > 0 ? Math.min(p, this.player.duration) : p;
  }

  go(view: View) {
    this.history.push(this.view);
    if (this.history.length > 30) this.history.shift();
    this.view = view;
    this.nowPlayingOpen = false;
  }

  back() {
    const prev = this.history.pop();
    if (prev) this.view = prev;
  }

  toast(text: string, kind: "info" | "error" = "info") {
    const id = ++this.toastId;
    this.toasts = [...this.toasts, { id, text, kind }];
    setTimeout(() => (this.toasts = this.toasts.filter((t) => t.id !== id)), kind === "error" ? 7000 : 3500);
  }

  async saveSettings(mut: (s: Settings) => void) {
    if (!this.settings) return;
    const copy = $state.snapshot(this.settings) as Settings;
    mut(copy);
    this.settings = copy;
    await api.saveSettings(copy);
  }

  sortSongs(list: Song[], sort = this.settings?.songSort ?? "title_asc"): Song[] {
    const out = [...list];
    switch (sort) {
      case "title_desc":
        return out.sort((a, b) => sortKey(b.title).localeCompare(sortKey(a.title)));
      case "artist_asc":
        return out.sort((a, b) => sortKey(a.artist || "~").localeCompare(sortKey(b.artist || "~")));
      case "date_desc":
        return out.sort((a, b) => b.mtime - a.mtime);
      case "date_asc":
        return out.sort((a, b) => a.mtime - b.mtime);
      case "duration_desc":
        return out.sort((a, b) => b.durationMs - a.durationMs);
      case "duration_asc":
        return out.sort((a, b) => a.durationMs - b.durationMs);
      default:
        return out.sort((a, b) => sortKey(a.title).localeCompare(sortKey(b.title)));
    }
  }

  play(list: Song[], index: number, context: string) {
    const local = list.every((s) => s.kind === "local");
    if (local) api.playIds(list.map((s) => s.id), index, context);
    else api.playSongs(list, index, context);
  }
}

function groupFolders(songs: Song[]): Folder[] {
  const map = new Map<string, Song[]>();
  for (const s of songs) {
    const l = map.get(s.folder);
    if (l) l.push(s);
    else map.set(s.folder, [s]);
  }
  return [...map.entries()].map(([path, list]) => ({
    path,
    name: folderName(path),
    songs: list,
    coverSong: list.find((s) => s.hasCover) ?? null,
  }));
}

export const store = new Store();
