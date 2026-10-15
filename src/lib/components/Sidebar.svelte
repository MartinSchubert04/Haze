<script lang="ts">
  import { Music2, Folder, TvMinimalPlay, Download, ListMusic, Settings, Star } from "@lucide/svelte";
  import { store, type View } from "../store.svelte";
  import { folderName } from "../format";

  const items: { view: View["name"]; label: string; icon: typeof Music2 }[] = [
    { view: "songs", label: "Canciones", icon: Music2 },
    { view: "folders", label: "Carpetas", icon: Folder },
    { view: "youtube", label: "YouTube", icon: TvMinimalPlay },
    { view: "downloads", label: "Descargas", icon: Download },
    { view: "queue", label: "Cola", icon: ListMusic },
  ];

  const active = $derived(store.view.name === "folder" ? "folders" : store.view.name);
  const running = $derived(store.downloads.filter((d) => d.status === "downloading" || d.status === "converting" || d.status === "queued").length);
  const favorites = $derived(store.settings?.downloadFolders ?? []);
</script>

<nav>
  <div class="brand" data-tauri-drag-region>
    <svg viewBox="0 0 32 32" width="22" height="22" aria-hidden="true">
      <defs>
        <linearGradient id="lg" x1="0" y1="0" x2="1" y2="1">
          <stop offset="0" stop-color="#f9e2af" /><stop offset=".5" stop-color="#fab387" /><stop offset="1" stop-color="#cba6f7" />
        </linearGradient>
      </defs>
      <circle cx="16" cy="16" r="11" fill="none" stroke="url(#lg)" stroke-width="3" stroke-linecap="round" stroke-dasharray="48 21" transform="rotate(-90 16 16)" />
      <path d="M13.5 11.5 L21 16 L13.5 20.5 Z" fill="url(#lg)" stroke="url(#lg)" stroke-width="1.5" stroke-linejoin="round" />
    </svg>
    <span>Reproductor</span>
  </div>

  <div class="group">
    {#each items as it}
      <button class="item" class:active={active === it.view} onclick={() => store.go({ name: it.view } as View)}>
        <it.icon size={18} strokeWidth={1.8} />
        <span>{it.label}</span>
        {#if it.view === "downloads" && running > 0}
          <span class="badge">{running}</span>
        {/if}
      </button>
    {/each}
  </div>

  {#if favorites.length}
    <div class="label">Carpetas favoritas</div>
    <div class="group favs">
      {#each favorites as path}
        <button
          class="item small"
          class:active={store.view.name === "folder" && store.view.path === path}
          title={path}
          onclick={() => store.go({ name: "folder", path })}
        >
          <Star size={14} strokeWidth={1.8} />
          <span class="ellipsis">{folderName(path)}</span>
        </button>
      {/each}
    </div>
  {/if}

  <div class="spacer"></div>

  <button class="item" class:active={active === "settings"} onclick={() => store.go({ name: "settings" })}>
    <Settings size={18} strokeWidth={1.8} />
    <span>Ajustes</span>
    {#if store.update}
      <span class="dot new" title="Hay una versión nueva"></span>
    {:else if store.settings?.ha.enabled}
      <span class="dot" class:ok={store.haStatus === "connected"} title="Home Assistant"></span>
    {/if}
  </button>
</nav>

<style>
  nav {
    width: 220px;
    flex: none;
    display: flex;
    flex-direction: column;
    padding: 14px 12px 16px;
    gap: 4px;
  }
  .brand {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 4px 10px 18px;
    font-weight: 650;
    letter-spacing: -0.01em;
    font-size: 15px;
  }
  .brand span {
    pointer-events: none;
  }
  .group {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .favs {
    overflow-y: auto;
    max-height: 40vh;
  }
  .label {
    margin: 22px 10px 6px;
    font-size: 11.5px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: var(--text-3);
  }
  .item {
    display: flex;
    align-items: center;
    gap: 12px;
    height: 38px;
    padding: 0 12px;
    border-radius: var(--radius);
    color: var(--text-2);
    font-weight: 520;
    text-align: left;
    transition: background 0.15s, color 0.15s;
    min-width: 0;
  }
  .item.small {
    height: 32px;
    gap: 10px;
    font-size: 13px;
  }
  .item:hover {
    background: rgba(255, 255, 255, 0.05);
    color: var(--text);
  }
  .item.active {
    background: rgba(255, 255, 255, 0.09);
    color: var(--text);
  }
  .item.active :global(svg) {
    color: var(--accent);
  }
  .badge {
    margin-left: auto;
    min-width: 20px;
    height: 20px;
    padding: 0 6px;
    border-radius: 10px;
    background: var(--accent);
    color: var(--accent-ink);
    font-size: 11.5px;
    font-weight: 700;
    display: grid;
    place-items: center;
  }
  .dot {
    margin-left: auto;
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: #e67878;
  }
  .dot.ok {
    background: #82c896;
  }
  .dot.new {
    background: var(--accent);
  }
  .spacer {
    flex: 1;
  }
</style>
