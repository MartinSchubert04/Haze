// Estado global de la interfaz. Rust es la fuente de verdad: esto es un espejo que se
// actualiza con los eventos que emite el backend.
import { listen } from "@tauri-apps/api/event";
import { folderName, sortKey } from "@haze/ui/format";
import { api, type DownloadJob, type PlayerState, type Settings, type Song, type UpdateInfo } from "./api";

export type Tab = "songs" | "folders" | "youtube" | "queue";

/**
 * Lo que se abre encima de las pestañas. Cada capa suma una entrada al historial, asi el
 * boton "atras" de Android cierra la de arriba en vez de salir de la app.
 */
export type Layer =
  | { kind: "folder"; path: string }
  | { kind: "nowPlaying" }
  | { kind: "settings" }
  | { kind: "song"; song: Song; queueIndex?: number }
  /** Un link pegado: escucharlo o bajarlo. */
  | { kind: "link"; url: string };

export type Folder = { path: string; name: string; songs: Song[]; coverSong: Song | null };

export const ALL_SONGS = "Todas las canciones";

const emptyState: PlayerState = {
  song: null,
  playing: false,
  buffering: false,
  duration: 0,
  volume: 100,
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
  access = $state(true);
  version = $state("");
  haStatus = $state("off");
  downloads = $state.raw<DownloadJob[]>([]);
  update = $state<UpdateInfo | null>(null);
  songs = $state.raw<Song[]>([]);
  settings = $state<Settings | null>(null);
  player = $state<PlayerState>(emptyState);
  scanning = $state(false);
  tab = $state<Tab>("songs");
  layers = $state<Layer[]>([]);
  toasts = $state<{ id: number; text: string; kind: "info" | "error" }[]>([]);

  // Posicion interpolada: Rust la manda cada ~500 ms y aca se completa entre medio.
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
    this.access = b.access || !b.ready;
    this.scanning = b.scanning;
    this.engineReady = b.ready;
    this.version = b.version;
    this.haStatus = b.haStatus;
    this.downloads = b.downloads;
    this.update = b.update;
    this.ready = true;

    listen<PlayerState>("player://state", (e) => {
      const prevErr = this.player.error;
      this.player = e.payload;
      this.setPosition(e.payload.position);
      if (e.payload.error && e.payload.error !== prevErr) this.toast(e.payload.error, "error");
    });
    listen<{ position: number; duration: number }>("player://position", (e) => this.setPosition(e.payload.position));
    listen<Song[]>("library://changed", (e) => (this.songs = e.payload));
    listen<boolean>("library://scanning", (e) => (this.scanning = e.payload));
    listen<UpdateInfo | null>("update://status", (e) => {
      const was = this.update?.status;
      this.update = e.payload;
      if (e.payload?.status === "ready" && was !== "ready") this.toast(`Haze ${e.payload.version} está lista: se instala al salir`);
    });
    listen<DownloadJob[]>("downloads://changed", (e) => (this.downloads = e.payload));
    listen<string>("downloads://done", (e) => this.toast(`Bajada: ${e.payload}`));
    listen<string>("ha://status", (e) => (this.haStatus = e.payload));
    listen<string>("app://error", (e) => this.toast(e.payload, "error"));
    listen<boolean>("app://ready", (e) => {
      this.engineReady = true;
      this.access = e.payload;
    });

    // El historial lleva cuantas capas habia: "atras" (o history.go) deja solo esas.
    history.replaceState({ depth: 0 }, "");
    window.addEventListener("popstate", (e) => {
      const depth = (e.state?.depth as number | undefined) ?? 0;
      if (depth < this.layers.length) this.layers = this.layers.slice(0, depth);
    });

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

  // ------------------------------------------------------------------ navegacion

  open(layer: Layer) {
    this.layers = [...this.layers, layer];
    history.pushState({ depth: this.layers.length }, "");
  }

  /** Cierra la capa de arriba, igual que el boton "atras". */
  close() {
    if (this.layers.length) history.back();
  }

  /** Cambiar de pestaña cierra todo lo que estaba abierto encima. */
  setTab(tab: Tab) {
    if (this.layers.length) history.go(-this.layers.length);
    this.tab = tab;
  }

  /** Cierra todo, pasa a Carpetas y abre esa. */
  async goToFolder(path: string) {
    const n = this.layers.length;
    if (n) {
      // Hay que esperar a que el historial vuelva antes de sumar la capa nueva.
      const back = new Promise((resolve) => window.addEventListener("popstate", resolve, { once: true }));
      history.go(-n);
      await back;
    }
    this.tab = "folders";
    this.open({ kind: "folder", path });
  }

  /** La carpeta abierta en la pestaña Carpetas, si hay. */
  openFolder = $derived.by(() => {
    for (let i = this.layers.length - 1; i >= 0; i--) {
      const l = this.layers[i];
      if (l.kind === "folder") return l.path;
    }
    return null;
  });

  isOpen(kind: Layer["kind"]) {
    return this.layers.some((l) => l.kind === kind);
  }

  // ------------------------------------------------------------------ acciones

  toast(text: string, kind: "info" | "error" = "info") {
    const id = ++this.toastId;
    this.toasts = [...this.toasts, { id, text, kind }];
    setTimeout(() => (this.toasts = this.toasts.filter((t) => t.id !== id)), kind === "error" ? 6000 : 2500);
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
      case "artist_asc":
        return out.sort((a, b) => sortKey(a.artist || "~").localeCompare(sortKey(b.artist || "~")));
      case "date_desc":
        return out.sort((a, b) => b.mtime - a.mtime);
      default:
        return out.sort((a, b) => sortKey(a.title).localeCompare(sortKey(b.title)));
    }
  }

  play(list: Song[], index: number, context: string) {
    if (list.some((s) => s.kind === "stream")) {
      api.playSongs(list, index, context);
    } else {
      api.playIds(
        list.map((s) => s.id),
        index,
        context,
      );
    }
  }

  async download(url: string, title: string, thumb: string | null, playlist = false) {
    await api.ytDownload(url, title, thumb, playlist);
    this.toast("Bajando a Music/Haze");
  }

  async enqueue(song: Song, next: boolean) {
    await api.enqueue(song, next);
    this.toast(next ? "Suena a continuación" : "Agregada a la cola");
  }
}

function groupFolders(songs: Song[]): Folder[] {
  const map = new Map<string, Song[]>();
  for (const s of songs) {
    const l = map.get(s.folder);
    if (l) l.push(s);
    else map.set(s.folder, [s]);
  }
  return [...map.entries()]
    .map(([path, list]) => ({
      path,
      name: folderName(path) || "Almacenamiento",
      songs: list,
      coverSong: list.find((s) => s.hasCover) ?? null,
    }))
    .sort((a, b) => sortKey(a.name).localeCompare(sortKey(b.name)));
}

export const store = new Store();
