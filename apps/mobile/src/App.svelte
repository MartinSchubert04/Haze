<script lang="ts">
  import { onMount } from "svelte";
  import { LoaderCircle } from "@lucide/svelte";
  import { store } from "./lib/store.svelte";
  import Background from "./lib/components/Background.svelte";
  import TabBar from "./lib/components/TabBar.svelte";
  import MiniPlayer from "./lib/components/MiniPlayer.svelte";
  import SongSheet from "./lib/components/SongSheet.svelte";
  import FolderSheet from "./lib/components/FolderSheet.svelte";
  import Access from "./lib/views/Access.svelte";
  import Songs from "./lib/views/Songs.svelte";
  import Folders from "./lib/views/Folders.svelte";
  import FolderDetail from "./lib/views/FolderDetail.svelte";
  import Queue from "./lib/views/Queue.svelte";
  import YouTube from "./lib/views/YouTube.svelte";
  import LinkSheet from "./lib/components/LinkSheet.svelte";
  import Settings from "./lib/views/Settings.svelte";
  import NowPlaying from "./lib/views/NowPlaying.svelte";

  let fatal = $state<string | null>(null);

  onMount(async () => {
    // Justo despues de instalar, el WebView a veces llama antes de que Rust registre los
    // comandos ("not allowed"): se reintenta un par de veces antes de dar error.
    for (let attempt = 1; ; attempt++) {
      try {
        await store.init();
        return;
      } catch (e) {
        if (attempt >= 4) {
          fatal = String(e);
          return;
        }
        await new Promise((r) => setTimeout(r, 500 * attempt));
      }
    }
  });

  // Las opciones de una cancion siempre se abren arriba de todo.
  const sheet = $derived(store.layers.at(-1));
</script>

<Background />

{#if fatal}
  <div class="empty"><h3>No se pudo cargar</h3><p>{fatal}</p></div>
{:else if !store.ready}
  <div class="boot"><LoaderCircle size={28} class="spin" /></div>
{:else if !store.access}
  <Access />
{:else}
  <div class="app">
    <main>
      {#if store.tab === "songs"}<Songs />
      {:else if store.tab === "folders"}<Folders />
      {:else if store.tab === "youtube"}<YouTube />
      {:else}<Queue />
      {/if}
    </main>

    {#if store.openFolder}<FolderDetail path={store.openFolder} />{/if}
    {#if store.isOpen("settings")}<Settings />{/if}

    <div class="dock">
      {#if store.player.song}<MiniPlayer />{/if}
      <TabBar />
    </div>

    {#if store.isOpen("nowPlaying") && store.player.song}<NowPlaying />{/if}
    {#if sheet?.kind === "song"}<SongSheet song={sheet.song} queueIndex={sheet.queueIndex} />{/if}
    {#if sheet?.kind === "folderOptions"}<FolderSheet path={sheet.path} />{/if}
    {#if sheet?.kind === "link"}<LinkSheet url={sheet.url} />{/if}
  </div>
{/if}

<div class="toasts">
  {#each store.toasts as t (t.id)}
    <div class="toast" class:error={t.kind === "error"}>{t.text}</div>
  {/each}
</div>

<style>
  .boot {
    height: 100%;
    display: grid;
    place-items: center;
    color: var(--text-2);
  }
  .app {
    position: relative;
    height: 100%;
    overflow: hidden;
  }
  main {
    height: 100%;
  }
  .dock {
    position: fixed;
    left: 0;
    right: 0;
    bottom: 0;
    z-index: 30;
    padding-bottom: var(--safe-bottom);
    background: linear-gradient(to top, color-mix(in srgb, var(--bg) 96%, transparent) 60%, color-mix(in srgb, var(--bg) 75%, transparent));
    backdrop-filter: var(--blur);
    border-top: 1px solid var(--line);
  }
  .toasts {
    position: fixed;
    left: 16px;
    right: 16px;
    bottom: calc(var(--tabbar) + var(--mini) + var(--safe-bottom) + 12px);
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 8px;
    z-index: 100;
    pointer-events: none;
  }
  .toast {
    max-width: 100%;
    padding: 10px 16px;
    border-radius: calc(12px * var(--rs));
    background: var(--pop);
    border: var(--border) solid var(--line-strong);
    box-shadow: var(--pop-shadow);
    font-size: 14px;
    animation: toast 0.2s ease-out;
  }
  .toast.error {
    border-color: color-mix(in srgb, var(--danger) 50%, transparent);
    color: color-mix(in srgb, var(--danger) 65%, var(--text));
  }
  @keyframes toast {
    from {
      opacity: 0;
      transform: translateY(6px);
    }
  }
</style>
