<script lang="ts">
  import { onMount } from "svelte";
  import { LoaderCircle } from "@lucide/svelte";
  import { store } from "./lib/store.svelte";
  import Background from "./lib/components/Background.svelte";
  import TabBar from "./lib/components/TabBar.svelte";
  import MiniPlayer from "./lib/components/MiniPlayer.svelte";
  import SongSheet from "./lib/components/SongSheet.svelte";
  import Access from "./lib/views/Access.svelte";
  import Songs from "./lib/views/Songs.svelte";
  import Folders from "./lib/views/Folders.svelte";
  import FolderDetail from "./lib/views/FolderDetail.svelte";
  import Queue from "./lib/views/Queue.svelte";
  import Settings from "./lib/views/Settings.svelte";
  import NowPlaying from "./lib/views/NowPlaying.svelte";

  let fatal = $state<string | null>(null);

  onMount(async () => {
    try {
      await store.init();
    } catch (e) {
      fatal = String(e);
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
    background: linear-gradient(to top, rgba(10, 13, 16, 0.96) 60%, rgba(10, 13, 16, 0.75));
    backdrop-filter: blur(24px) saturate(140%);
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
    border-radius: 12px;
    background: rgba(30, 34, 39, 0.96);
    border: 1px solid var(--line-strong);
    box-shadow: 0 10px 30px rgba(0, 0, 0, 0.35);
    font-size: 14px;
    animation: toast 0.2s ease-out;
  }
  .toast.error {
    border-color: rgba(243, 139, 168, 0.5);
    color: #f5c2d0;
  }
  @keyframes toast {
    from {
      opacity: 0;
      transform: translateY(6px);
    }
  }
</style>
