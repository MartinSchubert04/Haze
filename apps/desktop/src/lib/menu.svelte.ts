// Menu contextual unico para toda la app.
import { revealItemInDir } from "@tauri-apps/plugin-opener";
import { ListEnd, ListPlus, FolderOpen, FolderSearch, Download, Link, Play } from "@lucide/svelte";
import { api, type Song } from "./api";
import { store } from "./store.svelte";

export type MenuItem = { label: string; icon?: typeof Play; action: () => void; danger?: boolean } | "sep";

class MenuState {
  open = $state<{ x: number; y: number; items: MenuItem[] } | null>(null);

  show(e: MouseEvent, items: MenuItem[]) {
    e.preventDefault();
    e.stopPropagation();
    this.open = { x: e.clientX, y: e.clientY, items };
  }

  close() {
    this.open = null;
  }
}

export const menu = new MenuState();

export function songMenu(e: MouseEvent, song: Song, play?: () => void) {
  const items: MenuItem[] = [];
  if (play) items.push({ label: "Reproducir", icon: Play, action: play });
  items.push(
    { label: "Reproducir a continuación", icon: ListPlus, action: () => queue(song, true) },
    { label: "Agregar a la cola", icon: ListEnd, action: () => queue(song, false) },
  );
  if (song.kind === "local") {
    items.push(
      "sep",
      { label: "Ir a la carpeta", icon: FolderSearch, action: () => store.go({ name: "folder", path: song.folder }) },
      { label: "Mostrar en el explorador", icon: FolderOpen, action: () => revealItemInDir(song.path.replaceAll("/", "\\")) },
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

async function queue(song: Song, next: boolean) {
  await api.enqueue(song, next);
  store.toast(next ? "Suena a continuación" : "Agregada a la cola");
}
