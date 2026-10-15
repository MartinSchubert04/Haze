<script lang="ts">
  import type { Snippet } from "svelte";
  import { Shuffle, Play, ArrowUpDown, Check } from "@lucide/svelte";
  import { api, type Song } from "../api";
  import { store } from "../store.svelte";
  import { menu } from "../menu.svelte";

  let {
    title,
    sub,
    songs,
    context,
    sortable = true,
    extra,
  }: { title: string; sub?: string; songs: Song[]; context: string; sortable?: boolean; extra?: Snippet } = $props();

  const SORTS: [string, string][] = [
    ["title_asc", "Título (A-Z)"],
    ["title_desc", "Título (Z-A)"],
    ["artist_asc", "Artista"],
    ["date_desc", "Más recientes"],
    ["date_asc", "Más viejas"],
    ["duration_desc", "Más largas"],
    ["duration_asc", "Más cortas"],
  ];

  function sortMenu(e: MouseEvent) {
    const cur = store.settings?.songSort;
    menu.show(
      e,
      SORTS.map(([k, label]) => ({
        label,
        icon: k === cur ? Check : undefined,
        action: () => store.saveSettings((s) => (s.songSort = k)),
      })),
    );
  }

  async function shuffle() {
    if (!songs.length) return;
    if (!store.player.shuffle) await api.control({ type: "shuffle", value: true });
    store.play(songs, Math.floor(Math.random() * songs.length), context);
  }

  async function playAll() {
    if (!songs.length) return;
    if (store.player.shuffle) await api.control({ type: "shuffle", value: false });
    store.play(songs, 0, context);
  }
</script>

<div class="head">
  <div class="top">
    <div class="titles">
      <h1 class="page-title ellipsis">{title}</h1>
      {#if sub}<div class="page-sub">{sub}</div>{/if}
    </div>
    {#if sortable}
      <button class="icon-btn" title="Ordenar" onclick={sortMenu}><ArrowUpDown size={17} /></button>
    {/if}
  </div>
  <div class="actions">
    <button class="pill" onclick={shuffle} disabled={!songs.length}><Shuffle size={16} color="var(--accent)" /> Aleatorio</button>
    <button class="pill primary" onclick={playAll} disabled={!songs.length}><Play size={16} fill="currentColor" /> Reproducir</button>
    {@render extra?.()}
  </div>
</div>

<style>
  .head {
    padding: 8px 0 14px;
  }
  .top {
    display: flex;
    align-items: flex-end;
    gap: 12px;
  }
  .titles {
    flex: 1;
    min-width: 0;
  }
  .top .icon-btn {
    margin-bottom: 16px;
  }
  .actions {
    display: flex;
    flex-wrap: wrap;
    gap: 10px;
  }
</style>
