// Llamadas al backend de Rust (apps/mobile/src-tauri). Los tipos son los de escritorio.
import { invoke, convertFileSrc } from "@tauri-apps/api/core";
import type { PlayerState, QueueEntry, QueueView, Settings, Song } from "@haze/ui/types";

export type { PlayerState, QueueEntry, QueueView, Settings, Song };

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
  control: (control: Control) => invoke("player_control", { control }),
  enqueue: (id: number, next: boolean) => invoke("enqueue", { id, next }),
  queue: () => invoke<QueueView>("queue_get"),
  queueAction: (action: "jump" | "remove" | "move" | "clear", index = 0, to?: number) =>
    invoke("queue_action", { action, index, to }),
  saveSettings: (settings: Settings) => invoke("settings_set", { settings }),
};

/** URL de la tapa: la miniatura remota o la del archivo (la sirve Rust desde MediaStore). */
export function coverUrl(song: Song | null | undefined): string | null {
  if (!song) return null;
  if (song.thumb) return song.thumb;
  if (song.hasCover) return convertFileSrc(String(song.id), "cover");
  return null;
}
