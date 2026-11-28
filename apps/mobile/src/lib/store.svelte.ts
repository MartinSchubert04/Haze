// Estado global de la interfaz. Rust es la fuente de verdad: esto es un espejo que se
// actualiza con los eventos que emite el backend.
import { listen } from "@tauri-apps/api/event";
import { onBackButtonPress } from "@tauri-apps/api/app";
import { folderName, sortKey } from "@haze/ui/format";
import { fileKey, listSongs, removedSet, uniqueFiles, visibleLists, visibleSongs } from "@haze/ui/library";
import {
  api,
  type DownloadJob,
  type List,
  type PlayerState,
  type Settings,
  type Song,
  type SyncReport,
  type UpdateInfo,
  type UserData,
  type UserOp,
} from "./api";

export type Tab = "songs" | "folders" | "lists" | "youtube" | "queue";

/** Lo que se abre encima de las pestañas. "Atras" cierra la de arriba. */
export type Layer =
  | { kind: "folder"; path: string }
  | { kind: "list"; id: string }
  | { kind: "nowPlaying" }
  /** Con `section`, abre Ajustes con esa seccion desplegada. */
  | { kind: "settings"; section?: string }
  /** Con `listId`, abierta desde esa lista: se puede quitar de ahi. */
  | { kind: "song"; song: Song; queueIndex?: number; listId?: string }
  /** Opciones de una carpeta: se abren al mantenerla apretada. */
  | { kind: "folderOptions"; path: string }
  /** Opciones de una lista (renombrar, borrar). */
  | { kind: "listOptions"; id: string }
  /** Elegir a que lista agregar estas canciones. */
  | { kind: "addToList"; songs: Song[] }
  /** Pedir el nombre: con `id` renombra esa lista; sin, crea una con `songs`. */
  | { kind: "listName"; id?: string; songs?: Song[] }
  /** Un link (pegado o de una busqueda): se lee y despues se escucha o se baja. Con
   *  `preview` se muestran titulo y tapa mientras se lee. */
  | { kind: "link"; url: string; preview?: Song };

/**
 * Algo que "atras" puede cerrar antes de tocar las capas o las pestañas (un desplegable,
 * una busqueda). Devuelve true si hizo algo.
 */
type BackHandler = { level: "popup" | "page"; run: () => boolean };

export type Folder = { path: string; name: string; songs: Song[]; coverSong: Song | null };

export const ALL_SONGS = "All songs";

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
  /** Todo lo que hay en el telefono, con las dadas de baja. */
  library = $state.raw<Song[]>([]);
  userdata = $state.raw<UserData>({ lists: [], removed: [] });
  syncReport = $state.raw<SyncReport>({ status: "", at: 0, downloaded: 0, uploaded: 0, failed: 0, error: null });
  syncProgress = $state<{ done: number; total: number } | null>(null);
  settings = $state<Settings | null>(null);
  player = $state<PlayerState>(emptyState);
  scanning = $state(false);
  tab = $state<Tab>("songs");
  layers = $state<Layer[]>([]);
  toasts = $state<{ id: number; text: string; kind: "info" | "error"; busy: boolean }[]>([]);

  // Posicion interpolada: Rust la manda cada ~500 ms y aca se completa entre medio.
  private posBase = 0;
  private posAt = 0;
  position = $state(0);

  removed = $derived(removedSet(this.userdata));
  /** Lo que se ve: sin las dadas de baja (siguen en el telefono, pero no se muestran). */
  songs = $derived(visibleSongs(this.library, this.removed));
  lists = $derived(visibleLists(this.userdata));
  folders = $derived.by(() => groupFolders(this.songs));
  /** Una entrada por archivo: lo que muestra "Canciones". Las carpetas usan `songs`. */
  uniqueSongs = $derived(uniqueFiles(this.songs));

  private toastId = 0;

  async init() {
    const b = await api.bootstrap();
    this.settings = b.settings;
    this.library = b.songs;
    this.userdata = b.userdata;
    this.syncReport = b.sync;
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
    listen<Song[]>("library://changed", (e) => (this.library = e.payload));
    listen<UserData>("userdata://changed", (e) => (this.userdata = e.payload));
    listen<SyncReport>("sync://status", (e) => {
      this.syncReport = e.payload;
      if (e.payload.status !== "running") this.syncProgress = null;
      this.syncDone(e.payload);
    });
    listen<{ done: number; total: number }>("sync://progress", (e) => (this.syncProgress = e.payload));
    listen<boolean>("library://scanning", (e) => (this.scanning = e.payload));
    listen<UpdateInfo | null>("update://status", (e) => {
      const was = this.update?.status;
      this.update = e.payload;
      if (this.updateToast !== null) this.followUpdate(e.payload);
      else if (e.payload?.status === "ready" && was !== "ready") this.toast(readyText(e.payload.version));
    });
    listen<DownloadJob[]>("downloads://changed", (e) => {
      // Una descarga que acaba de fallar: se avisa, no solo en la lista de YouTube.
      const before = new Map(this.downloads.map((d) => [d.id, d]));
      for (const d of e.payload) {
        if (d.error && !before.get(d.id)?.error) this.toast(`Couldn't download ${d.title}: ${d.error}`, "error");
      }
      this.downloads = e.payload;
    });
    listen<string>("downloads://done", (e) => this.toast(`Downloaded: ${e.payload}`));
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

  /** Cambia la capa de arriba por otra, de una vez (sin quedar un momento sin ninguna). */
  replace(layer: Layer) {
    this.layers = [...this.layers.slice(0, -1), layer];
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

  /** La lista abierta en la pestaña Listas, si hay. */
  openList = $derived.by(() => {
    for (let i = this.layers.length - 1; i >= 0; i--) {
      const l = this.layers[i];
      if (l.kind === "list") return l.id;
    }
    return null;
  });

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

  private toastTimers = new Map<number, ReturnType<typeof setTimeout>>();

  /** Muestra un aviso. Con `busy` queda (con una ruedita) hasta que se cambie con `setToast`. */
  toast(text: string, kind: "info" | "error" = "info", busy = false) {
    const id = ++this.toastId;
    this.toasts = [...this.toasts, { id, text, kind, busy }];
    if (!busy) this.expire(id, kind);
    return id;
  }

  /** Cambia el texto de un aviso que sigue andando (el avance de una descarga). */
  busyToast(id: number, text: string) {
    this.toasts = this.toasts.map((t) => (t.id === id ? { ...t, text } : t));
  }

  /** Cambia un aviso por otro en el mismo lugar (de "Syncing…" a como salio). */
  setToast(id: number, text: string, kind: "info" | "error" = "info") {
    if (!this.toasts.some((t) => t.id === id)) return void this.toast(text, kind);
    this.toasts = this.toasts.map((t) => (t.id === id ? { id, text, kind, busy: false } : t));
    this.expire(id, kind);
  }

  dropToast(id: number) {
    clearTimeout(this.toastTimers.get(id));
    this.toastTimers.delete(id);
    this.toasts = this.toasts.filter((t) => t.id !== id);
  }

  private expire(id: number, kind: "info" | "error") {
    clearTimeout(this.toastTimers.get(id));
    this.toastTimers.set(id, setTimeout(() => this.dropToast(id), kind === "error" ? 6000 : 2500));
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
    const byTitle = (a: Song, b: Song) => sortKey(a.title).localeCompare(sortKey(b.title));
    switch (sort) {
      case "title_desc":
        return out.sort((a, b) => byTitle(b, a));
      case "artist_asc":
        return out.sort((a, b) => sortKey(a.artist || "~").localeCompare(sortKey(b.artist || "~")) || byTitle(a, b));
      case "date_desc":
        return out.sort((a, b) => b.mtime - a.mtime);
      case "date_asc":
        return out.sort((a, b) => a.mtime - b.mtime);
      case "duration_desc":
        return out.sort((a, b) => b.durationMs - a.durationMs || byTitle(a, b));
      case "duration_asc":
        return out.sort((a, b) => a.durationMs - b.durationMs || byTitle(a, b));
      default:
        return out.sort(byTitle);
    }
  }

  // ------------------------------------------------------------------ menu del engranaje

  /** Secciones de Ajustes desplegadas: quedan como estaban al volver a entrar. */
  settingsOpen = $state<Record<string, boolean>>({});

  /**
   * Se pidio a mano: el aviso "Syncing…" que queda en pantalla hasta que la vuelta
   * termine y ahi pasa a decir como salio (un solo toast, no dos seguidos).
   */
  private syncToast: { id: number; at: number } | null = null;

  /** "Sync" del menu: sincroniza ya y avisa con un toast como salio. */
  async syncNow() {
    const s = this.settings?.sync;
    if (!s?.enabled || !s.server.trim()) {
      this.toast("Sync is off. Set it up in Settings");
      this.open({ kind: "settings", section: "sync" });
      return;
    }
    // Ya hay una vuelta andando (la automatica o una pedida antes): no se pide otra.
    if (this.syncToast || this.syncReport.status === "running") {
      if (!this.syncToast) this.toast("Already syncing…");
      return;
    }
    if (this.syncReport.status === "network" && !(await api.requestLocalNetwork().catch(() => false))) {
      this.toast("Haze needs the local network permission to sync", "error");
      return;
    }
    const t = { id: this.toast("Syncing…", "info", true), at: Date.now() };
    this.syncToast = t;
    // Si Rust no contesta (no pudo ni empezar), el aviso no queda para siempre.
    setTimeout(() => {
      if (this.syncToast !== t) return;
      this.syncToast = null;
      this.dropToast(t.id);
    }, 5 * 60_000);
    try {
      await api.syncNow();
    } catch (e) {
      this.syncToast = null;
      this.setToast(t.id, `Sync failed: ${e}`, "error");
    }
  }

  /** Al terminar una vuelta pedida desde el menu, el "Syncing…" pasa a decir como salio. */
  private syncDone(r: SyncReport) {
    const t = this.syncToast;
    if (!t || r.status === "running") return;
    this.syncToast = null;
    let text: string;
    let kind: "info" | "error" = "info";
    if (r.status === "ok") {
      const parts = [r.downloaded && `${r.downloaded} downloaded`, r.uploaded && `${r.uploaded} uploaded`].filter(Boolean);
      text = parts.length ? `Synced: ${parts.join(", ")}` : "Everything's up to date";
    } else if (r.status === "away") text = "Couldn't find the server. Are you home?";
    else if (r.status === "network") (text = "Haze needs the local network permission to sync"), (kind = "error");
    else if (r.status === "error") (text = `Sync failed: ${r.error ?? "something went wrong"}`), (kind = "error");
    else return this.dropToast(t.id);
    // Si contesta enseguida, "Syncing…" se deja ver un momento antes de cambiar: si no
    // parece que salieron dos avisos de golpe.
    const wait = Math.max(0, 900 - (Date.now() - t.at));
    setTimeout(() => this.setToast(t.id, text, kind), wait);
  }

  checkingUpdate = $state(false);

  /**
   * "Update" pedido desde el menu: un solo aviso que va de "Checking…" a "Downloading…
   * 42%" y termina en "ready" (o en que no hay nada nuevo). Rust recien contesta
   * `updateCheck` con el APK ya bajado: el avance llega por `update://status`.
   */
  private updateToast: number | null = null;

  /** "Update" del menu: busca ya y muestra como va en un toast. */
  async checkUpdate() {
    if (this.update?.status === "ready") return this.installUpdate();
    if (this.updateToast !== null && this.toasts.some((t) => t.id === this.updateToast)) return;
    if (this.update) {
      // Ya se estaba bajando sola: el aviso la sigue hasta que este lista.
      this.updateToast = this.toast("", "info", true);
      this.followUpdate(this.update);
      return;
    }
    if (this.checkingUpdate) return;
    this.checkingUpdate = true;
    const id = this.toast("Checking for updates…", "info", true);
    this.updateToast = id;
    const at = Date.now();
    // Si contesta enseguida, "Checking…" se deja ver un momento antes de cambiar.
    const settle = (text: string, kind: "info" | "error" = "info") => {
      if (this.updateToast !== id) return;
      this.updateToast = null;
      setTimeout(() => this.setToast(id, text, kind), Math.max(0, 900 - (Date.now() - at)));
    };
    try {
      const v = await api.updateCheck();
      if (!v && !this.update) settle(`You're on the latest version (${this.version})`);
    } catch (e) {
      settle(`Couldn't update: ${e}`, "error");
    } finally {
      this.checkingUpdate = false;
    }
  }

  /** Lleva el aviso de "Update" al estado de la descarga. */
  private followUpdate(u: UpdateInfo | null) {
    const id = this.updateToast;
    if (id === null) return;
    if (!u) {
      // Fallo la descarga. Si la pidio este toque, el error con detalle llega por `updateCheck`.
      if (!this.checkingUpdate) (this.updateToast = null), this.setToast(id, "Couldn't download the update", "error");
      return;
    }
    if (u.status === "ready") {
      this.updateToast = null;
      this.setToast(id, readyText(u.version));
    } else {
      this.busyToast(id, `Downloading Haze ${u.version}… ${Math.round(u.progress * 100)}%`);
    }
  }

  async installUpdate() {
    try {
      if (!(await api.updateInstall())) this.toast('Allow "Install unknown apps" for Haze and tap again');
    } catch (e) {
      this.toast(String(e), "error");
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

  /** Pide la descarga. Devuelve su id (para seguirla en `downloads`) o null si fallo. */
  async download(url: string, title: string, thumb: string | null, playlist = false): Promise<number | null> {
    // Encolar contesta enseguida: si no, algo quedo trabado y se avisa igual.
    const timeout = new Promise<never>((_, reject) =>
      setTimeout(() => reject("the app didn't respond"), 15000),
    );
    try {
      const id = await Promise.race([api.ytDownload(url, title, thumb, playlist), timeout]);
      this.toast("Downloading to Music/Haze");
      return id;
    } catch (e) {
      this.toast(`Couldn't start downloading ${title}: ${e}`, "error");
      return null;
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
    this.library = this.library.filter((s) => s.folder !== path);
    await this.saveSettings((x) => (x.hiddenFolders = [...new Set([...(x.hiddenFolders ?? []), path])]));
    this.toast("Folder hidden. You can bring it back in Settings");
  }

  async showFolder(path: string) {
    await this.saveSettings((x) => (x.hiddenFolders = (x.hiddenFolders ?? []).filter((p) => p !== path)));
  }

  async enqueue(song: Song, next: boolean) {
    await api.enqueue(song, next);
    this.toast(next ? "Playing next" : "Added to queue");
  }

  // ------------------------------------------------------------------ listas y bajas

  list(id: string): List | undefined {
    return this.lists.find((l) => l.id === id);
  }

  /** Las canciones de la lista que hay en el telefono, en su orden. */
  listSongs(list: List): Song[] {
    return listSongs(list, this.songs);
  }

  /** Rust guarda, avisa con "userdata://changed" y, en casa, lo manda al server. */
  async editUser(op: UserOp): Promise<string | null> {
    try {
      return await api.editUser(op);
    } catch (e) {
      this.toast(`Couldn't save: ${e}`, "error");
      return null;
    }
  }

  async createList(name: string, songs: Song[] = []) {
    const id = await this.editUser({ type: "create", name, files: songs.map(fileKey) });
    if (id) this.toast(songs.length ? `Added to ${name.trim()}` : `Created ${name.trim()}`);
    return id;
  }

  /** Borra la lista y cierra lo que estaba abierto de ella. */
  async deleteList(list: List) {
    this.layers = this.layers.filter((l) => !((l.kind === "list" || l.kind === "listOptions") && l.id === list.id));
    await this.editUser({ type: "delete", id: list.id });
    this.toast(`Deleted ${list.name}`);
  }

  async addToList(list: List, songs: Song[]) {
    await this.editUser({ type: "add", id: list.id, files: songs.map(fileKey) });
    this.toast(`Added to ${list.name}`);
  }

  /** Oculta las canciones en todos los aparatos. El archivo no se borra. */
  async removeSongs(songs: Song[]) {
    await this.editUser({ type: "setRemoved", files: songs.map((s) => [fileKey(s), s.title]), removed: true });
    this.toast("Removed. You can restore it in Settings");
  }

  async restore(file: number) {
    await this.editUser({ type: "setRemoved", files: [[file, ""]], removed: false });
  }
}

const readyText = (version: string) => `Haze ${version} is ready: it installs when you leave the app`;

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
      name: folderName(path) || "Storage",
      songs: list,
      coverSong: list.find((s) => s.hasCover) ?? null,
    }))
    .sort((a, b) => sortKey(a.name).localeCompare(sortKey(b.name)));
}

export const store = new Store();
