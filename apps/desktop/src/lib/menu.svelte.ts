// Menu contextual unico para toda la app.
import { revealItemInDir } from "@tauri-apps/plugin-opener";
import { ListEnd, ListPlus, FolderOpen, FolderSearch, Download, Link, Play, Library, Plus, EyeOff } from "@lucide/svelte";
import { api, type Song } from "./api";
import { store } from "./store.svelte";

export type MenuItem = { label: string; icon?: typeof Play; action: () => void; danger?: boolean } | "sep";

class MenuState {
  open = $state<{ x: number; y: number; items: MenuItem[] } | null>(null);

  show(e: MouseEvent, items: MenuItem[]) {
    e.preventDefault();
    e.stopPropagation();
    this.showAt(e.clientX, e.clientY, items);
  }

  showAt(x: number, y: number, items: MenuItem[]) {
    this.open = { x, y, items };
  }

  close() {
    this.open = null;
  }
}

export const menu = new MenuState();

/** `extra`: lo propio de la vista (por ejemplo, "Quitar de la lista"). */
export function songMenu(e: MouseEvent, song: Song, play?: () => void, extra: MenuItem[] = []) {
  const items: MenuItem[] = [];
  if (play) items.push({ label: "Reproducir", icon: Play, action: play });
  items.push(
    { label: "Reproducir a continuación", icon: ListPlus, action: () => queue(song, true) },
    { label: "Agregar a la cola", icon: ListEnd, action: () => queue(song, false) },
  );
  if (song.kind === "local") {
    const at = { x: e.clientX, y: e.clientY };
    items.push(
      "sep",
      // El menu se cierra al elegir: el de las listas se abre en el mismo lugar.
      { label: "Agregar a lista…", icon: Library, action: () => setTimeout(() => listPicker(at.x, at.y, [song])) },
      ...extra,
      "sep",
      { label: "Ir a la carpeta", icon: FolderSearch, action: () => store.go({ name: "folder", path: song.folder }) },
      { label: "Mostrar en el explorador", icon: FolderOpen, action: () => revealItemInDir(song.path.replaceAll("/", "\\")) },
      "sep",
      { label: "Dar de baja", icon: EyeOff, danger: true, action: () => store.removeSongs([song]) },
    );
  } else {
    items.push(
      "sep",
      { label: "Bajar como MP3", icon: Download, action: () => (store.pendingLink = song.path) },
      {
        label: "Copiar link",
        icon: Link,
        action: () => {
          navigator.clipboard.writeText(song.path);
          store.toast("Link copiado");
        },
      },
    );
  }
  menu.show(e, items);
}

/** Elegir a que lista agregar, o crear una con estas canciones. */
export function listPicker(x: number, y: number, songs: Song[]) {
  const items: MenuItem[] = store.lists.map((l) => ({ label: l.name, icon: Library, action: () => store.addToList(l, songs) }));
  if (items.length) items.push("sep");
  items.push({ label: "Nueva lista…", icon: Plus, action: () => store.newList(songs) });
  menu.showAt(x, y, items);
}

async function queue(song: Song, next: boolean) {
  await api.enqueue(song, next);
  store.toast(next ? "Suena a continuación" : "Agregada a la cola");
}
