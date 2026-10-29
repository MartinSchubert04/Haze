// Estado global de la interfaz. Rust es la fuente de verdad: esto es un espejo que se
// actualiza con los eventos que emite el backend.
import { listen } from "@tauri-apps/api/event";
import { onBackButtonPress } from "@tauri-apps/api/app";
import { folderName, sortKey } from "@haze/ui/format";
import { uniqueFiles } from "@haze/ui/library";
import { api, type DownloadJob, type PlayerState, type Settings, type Song, type UpdateInfo } from "./api";

export type Tab = "songs" | "folders" | "youtube" | "queue";

/** Lo que se abre encima de las pestañas. "Atras" cierra la de arriba. */
export type Layer =
  | { kind: "folder"; path: string }
  | { kind: "nowPlaying" }
  | { kind: "settings" }
  | { kind: "song"; song: Song; queueIndex?: number }
  /** Opciones de una carpeta: se abren al mantenerla apretada. */
  | { kind: "folderOptions"; path: string }
  /** Un link pegado: escucharlo o bajarlo. */
  | { kind: "link"; url: string };

/**
 * Algo que "atras" puede cerrar antes de tocar las capas o las pestañas (un desplegable,
 * una busqueda). Devuelve true si hizo algo.
 */
type BackHandler = { level: "popup" | "page"; run: () => boolean };

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
  /** Una entrada por archivo: lo que muestra "Canciones". Las carpetas usan `songs`. */
  uniqueSongs = $derived(uniqueFiles(this.songs));

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
    listen<DownloadJob[]>("downloads://changed", (e) => {
      // Una descarga que acaba de fallar: se avisa, no solo en la lista de YouTube.
      const before = new Map(this.downloads.map((d) => [d.id, d]));
      for (const d of e.payload) {
        if (d.error && !before.get(d.id)?.error) this.toast(`No se pudo bajar ${d.title}: ${d.error}`, "error");
      }
      this.downloads = e.payload;
    });
    listen<string>("downloads://done", (e) => this.toast(`Bajada: ${e.payload}`));
    listen<string>("ha://status", (e) => (this.haStatus = e.payload));
    listen<string>("app://error", (e) => this.toast(e.payload, "error"));
    listen<boolean>("app://ready", (e) => {
      this.engineReady = true;
      this.access = e.payload;
    });

    // El boton "atras" de Android lo maneja la app (no el historial del WebView, que
    // saltea entradas que no salieron de un toque y terminaba cerrando la app).
    onBackButtonPress(() => this.back()).catch(() => {});
    window.addEventListener("keydown", (e) => {
      if (e.key === "Escape") this.back();
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

  /** Pestañas por las que se paso, sin repetir: "atras" vuelve a la anterior. */
  private tabs: Tab[] = ["songs"];
  private backHandlers: BackHandler[] = [];

  open(layer: Layer) {
    this.layers = [...this.layers, layer];
  }

  /** Cierra la capa de arriba. */
  close() {
    if (this.layers.length) this.layers = this.layers.slice(0, -1);
  }

  /**
   * Registra algo que "atras" cierra primero: un "popup" va antes que las capas, una
   * "page" (la busqueda de una pestaña) despues. Devuelve la funcion para sacarlo.
   */
  onBack(level: BackHandler["level"], run: () => boolean) {
    const h: BackHandler = { level, run };
    this.backHandlers.push(h);
    return () => {
      this.backHandlers = this.backHandlers.filter((x) => x !== h);
    };
  }

  private runHandlers(level: BackHandler["level"]) {
    for (let i = this.backHandlers.length - 1; i >= 0; i--) {
      const h = this.backHandlers[i];
      if (h.level === level && h.run()) return true;
    }
    return false;
  }

  /**
   * Boton "atras": cierra lo de mas arriba (desplegable, capa, busqueda), si no vuelve a
   * la pestaña anterior, y solo desde Canciones sin nada abierto manda la app al fondo.
   */
  back() {
    if (this.runHandlers("popup")) return;
    if (this.layers.length) return this.close();
    if (this.runHandlers("page")) return;
    if (this.tabs.length > 1) {
      this.tabs = this.tabs.slice(0, -1);
      this.tab = this.tabs.at(-1)!;
      return;
    }
    api.minimize();
  }

  /**
   * Cambiar de pestaña cierra lo que estaba abierto encima. La pestaña nueva se apila y
   * "atras" vuelve a la anterior; si ya estaba en la pila, se vuelve hasta ella, asi la
   * pila no crece y desde Canciones "atras" sale de la app.
   */
  setTab(tab: Tab) {
    this.layers = [];
    if (tab === this.tab) return;
    const i = this.tabs.indexOf(tab);
    this.tabs = i >= 0 ? this.tabs.slice(0, i + 1) : [...this.tabs, tab];
    this.tab = tab;
  }

  /** Cierra todo, pasa a Carpetas y abre esa. */
  goToFolder(path: string) {
    this.setTab("folders");
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

  sortSongs(list: Song[], sort = this.settings?.songSort ?? "date_desc"): Song[] {
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
    // Encolar contesta enseguida: si no, algo quedo trabado y se avisa igual.
    const timeout = new Promise<never>((_, reject) =>
      setTimeout(() => reject("la app no respondió"), 15000),
    );
    try {
      await Promise.race([api.ytDownload(url, title, thumb, playlist), timeout]);
      this.toast("Bajando a Music/Haze");
    } catch (e) {
      this.toast(`No se pudo empezar a bajar ${title}: ${e}`, "error");
    }
  }

  /**
   * Un tema elegido a mano (de una busqueda): suena ya y despues sigue la cola que habia.
   * Si no sonaba nada, arranca desde ahi con toda la biblioteca.
   */
  playNow(song: Song) {
    if (this.player.song) {
      api.playNow(song);
      return;
    }
    const all = this.sortSongs(this.uniqueSongs);
    this.play(all, Math.max(0, all.findIndex((s) => s.id === song.id)), ALL_SONGS);
  }

  /** Quita la carpeta de la biblioteca (no borra nada del telefono). */
  async hideFolder(path: string) {
    this.layers = [];
    this.songs = this.songs.filter((s) => s.folder !== path);
    await this.saveSettings((x) => (x.hiddenFolders = [...new Set([...(x.hiddenFolders ?? []), path])]));
    this.toast("Carpeta quitada. Se vuelve a mostrar desde Ajustes");
  }

  async showFolder(path: string) {
    await this.saveSettings((x) => (x.hiddenFolders = (x.hiddenFolders ?? []).filter((p) => p !== path)));
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
