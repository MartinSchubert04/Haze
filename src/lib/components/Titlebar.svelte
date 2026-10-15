<script lang="ts">
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { Minus, Square, X, Search, PictureInPicture2, ChevronLeft } from "@lucide/svelte";
  import { api } from "../api";
  import { store } from "../store.svelte";

  const win = getCurrentWindow();
  let input: HTMLInputElement;

  function onKey(e: KeyboardEvent) {
    if ((e.ctrlKey && (e.key === "f" || e.key === "k")) || (e.key === "/" && !isTyping(e))) {
      e.preventDefault();
      input?.focus();
      input?.select();
    }
  }

  function isTyping(e: KeyboardEvent) {
    const t = e.target as HTMLElement;
    return t.tagName === "INPUT" || t.tagName === "TEXTAREA";
  }

  function submit(e: Event) {
    e.preventDefault();
    // Enter en la busqueda: si no hay nada local, se busca en YouTube.
    const q = store.search.trim();
    if (!q) return;
    const local = store.songs.some((s) => s.title.toLowerCase().includes(q.toLowerCase()));
    if (!local || store.view.name === "youtube") store.go({ name: "youtube", query: q });
  }
</script>

<svelte:window onkeydown={onKey} />

<header data-tauri-drag-region>
  <div class="left" data-tauri-drag-region>
    <button class="icon-btn small" title="Atrás" onclick={() => store.back()} disabled={store.history.length === 0}>
      <ChevronLeft size={18} />
    </button>
  </div>

  <form class="search" onsubmit={submit}>
    <Search size={15} />
    <input
      bind:this={input}
      type="search"
      placeholder="Buscar canciones, carpetas o en YouTube"
      bind:value={store.search}
      onfocus={() => {
        if (store.view.name !== "songs" && store.view.name !== "youtube" && store.view.name !== "folders") store.go({ name: "songs" });
      }}
    />
    <kbd>Ctrl K</kbd>
  </form>

  <div class="right" data-tauri-drag-region>
    <button class="win" title="Mini reproductor" onclick={() => api.openMini()}><PictureInPicture2 size={15} /></button>
    <button class="win" title="Minimizar" onclick={() => win.minimize()}><Minus size={16} /></button>
    <button class="win" title="Maximizar" onclick={() => win.toggleMaximize()}><Square size={13} /></button>
    <button class="win close" title="Cerrar" onclick={() => win.close()}><X size={17} /></button>
  </div>
</header>

<style>
  header {
    height: 52px;
    display: grid;
    grid-template-columns: 1fr minmax(260px, 520px) 1fr;
    align-items: center;
    gap: 12px;
    padding: 0 0 0 12px;
  }
  .left {
    display: flex;
    align-items: center;
    height: 100%;
  }
  .small {
    width: 30px;
    height: 30px;
  }
  .small:disabled {
    opacity: 0.3;
    pointer-events: none;
  }
  .search {
    display: flex;
    align-items: center;
    gap: 8px;
    height: 34px;
    padding: 0 6px 0 12px;
    border-radius: 999px;
    background: rgba(255, 255, 255, 0.06);
    border: 1px solid var(--line);
    color: var(--text-3);
    transition: background 0.15s, border-color 0.15s;
  }
  .search:focus-within {
    background: rgba(255, 255, 255, 0.09);
    border-color: var(--line-strong);
    color: var(--text-2);
  }
  .search input {
    flex: 1;
    min-width: 0;
    background: none;
    border: 0;
    padding: 0;
    font-size: 13.5px;
  }
  .search input::-webkit-search-cancel-button {
    filter: invert(1) opacity(0.5);
  }
  kbd {
    font: inherit;
    font-size: 11px;
    color: var(--text-3);
    border: 1px solid var(--line);
    border-radius: 6px;
    padding: 2px 6px;
  }
  .right {
    display: flex;
    justify-content: flex-end;
    height: 100%;
  }
  .win {
    width: 46px;
    height: 100%;
    display: grid;
    place-items: center;
    color: var(--text-2);
    transition: background 0.12s, color 0.12s;
  }
  .win:hover {
    background: rgba(255, 255, 255, 0.08);
    color: var(--text);
  }
  .win.close:hover {
    background: #c42b1c;
    color: #fff;
  }
</style>
