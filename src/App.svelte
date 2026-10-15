<script lang="ts">
  import { onMount } from "svelte";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { LoaderCircle } from "@lucide/svelte";
  import { api, isUrl } from "./lib/api";
  import { store } from "./lib/store.svelte";
  import Background from "./lib/components/Background.svelte";
  import Titlebar from "./lib/components/Titlebar.svelte";
  import Sidebar from "./lib/components/Sidebar.svelte";
  import PlayerBar from "./lib/components/PlayerBar.svelte";
  import ContextMenu from "./lib/components/ContextMenu.svelte";
  import LinkDialog from "./lib/components/LinkDialog.svelte";
  import Songs from "./lib/views/Songs.svelte";
  import Folders from "./lib/views/Folders.svelte";
  import FolderDetail from "./lib/views/FolderDetail.svelte";
  import YouTube from "./lib/views/YouTube.svelte";
  import Downloads from "./lib/views/Downloads.svelte";
  import Queue from "./lib/views/Queue.svelte";
  import Settings from "./lib/views/Settings.svelte";
  import NowPlaying from "./lib/views/NowPlaying.svelte";

  let fatal = $state<string | null>(null);

  onMount(async () => {
    // La ventana nace oculta para no mostrar un cuadro vacio mientras arranca el WebView.
    const win = getCurrentWindow();
    win.show().then(() => win.setFocus());
    try {
      await store.init();
      store.checkUpdate();
    } catch (e) {
      fatal = String(e);
    }
  });

  function typing(e: Event) {
    const t = e.target as HTMLElement;
    return t.tagName === "INPUT" || t.tagName === "TEXTAREA" || t.isContentEditable;
  }

  function onKey(e: KeyboardEvent) {
    if (typing(e) || store.pendingLink) return;
    if (e.code === "Space") {
      e.preventDefault();
      api.control({ type: "toggle" });
    } else if (e.ctrlKey && e.key === "ArrowRight") {
      api.control({ type: "next" });
    } else if (e.ctrlKey && e.key === "ArrowLeft") {
      api.control({ type: "previous" });
    } else if (e.key === "ArrowRight" && store.player.song) {
      api.control({ type: "seek", value: store.position + 10 });
    } else if (e.key === "ArrowLeft" && store.player.song) {
      api.control({ type: "seek", value: Math.max(0, store.position - 10) });
    } else if (e.ctrlKey && e.key === "ArrowUp") {
      api.control({ type: "volume", value: Math.min(100, store.player.volume + 5) });
    } else if (e.ctrlKey && e.key === "ArrowDown") {
      api.control({ type: "volume", value: Math.max(0, store.player.volume - 5) });
    }
  }

  // Pegar un link en cualquier lado (fuera de un campo de texto) abre el dialogo.
  function onPaste(e: ClipboardEvent) {
    if (typing(e)) return;
    const text = e.clipboardData?.getData("text")?.trim() ?? "";
    if (isUrl(text)) {
      e.preventDefault();
      store.pendingLink = text;
    }
  }

  // Tambien se puede arrastrar un link del navegador a la ventana.
  function onDrop(e: DragEvent) {
    const text = e.dataTransfer?.getData("text/uri-list") || e.dataTransfer?.getData("text/plain") || "";
    if (isUrl(text.split("\n")[0])) {
      e.preventDefault();
      store.pendingLink = text.split("\n")[0].trim();
    }
  }
</script>

<svelte:window onkeydown={onKey} onpaste={onPaste} ondragover={(e) => e.preventDefault()} ondrop={onDrop} />

<Background />

{#if fatal}
  <div class="empty" data-tauri-drag-region><h3>No se pudo cargar</h3><p>{fatal}</p></div>
{/if}

{#if store.ready}
  <div class="app">
    <Titlebar />
    <div class="middle">
      <Sidebar />
      <main>
        {#key store.view}
          <div class="view">
            {#if store.view.name === "songs"}<Songs />
            {:else if store.view.name === "folders"}<Folders />
            {:else if store.view.name === "folder"}<FolderDetail path={store.view.path} />
            {:else if store.view.name === "youtube"}<YouTube query={store.view.query} />
            {:else if store.view.name === "downloads"}<Downloads />
            {:else if store.view.name === "queue"}<Queue />
            {:else if store.view.name === "settings"}<Settings />
            {/if}
          </div>
        {/key}
      </main>
      {#if store.nowPlayingOpen && store.player.song}<NowPlaying />{/if}
    </div>
    <PlayerBar />
  </div>

  {#if store.toolProgress || !store.engineReady}
    <div class="setup glass">
      <LoaderCircle size={16} class="spin" />
      {#if store.toolProgress}
        Bajando {store.toolProgress.tool}… {Math.round(store.toolProgress.progress * 100)}%
      {:else}
        Preparando el motor de audio…
      {/if}
    </div>
  {/if}

  <div class="toasts">
    {#each store.toasts as t (t.id)}
      <div class="toast" class:error={t.kind === "error"}>{t.text}</div>
    {/each}
  </div>

  <ContextMenu />
  <LinkDialog />
{/if}

<style>
  .app {
    height: 100vh;
    display: flex;
    flex-direction: column;
  }
  .middle {
    position: relative;
    flex: 1;
    min-height: 0;
    display: flex;
  }
  main {
    flex: 1;
    min-width: 0;
    margin: 0 10px 10px 0;
    border-radius: var(--radius-lg);
    background: rgba(255, 255, 255, 0.025);
    border: 1px solid rgba(255, 255, 255, 0.05);
    overflow: hidden;
  }
  .view {
    position: relative;
    height: 100%;
    animation: in 0.18s ease-out;
  }
  @keyframes in {
    from {
      opacity: 0;
      transform: translateY(4px);
    }
  }
  .setup {
    position: fixed;
    right: 20px;
    bottom: 112px;
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 10px 16px;
    border-radius: 999px;
    font-size: 13px;
    z-index: 60;
  }
  .toasts {
    position: fixed;
    left: 50%;
    bottom: 112px;
    transform: translateX(-50%);
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 8px;
    z-index: 100;
    pointer-events: none;
  }
  .toast {
    max-width: min(560px, 80vw);
    padding: 10px 16px;
    border-radius: 12px;
    background: rgba(30, 34, 39, 0.94);
    border: 1px solid var(--line-strong);
    box-shadow: 0 10px 30px rgba(0, 0, 0, 0.35);
    font-size: 13.5px;
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
