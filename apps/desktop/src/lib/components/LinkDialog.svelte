<script lang="ts">
  // Lo que aparece al pegar un link: escucharlo ya o bajarlo como MP3 a una carpeta.
  import { open } from "@tauri-apps/plugin-dialog";
  import { X, Download, Play, FolderPlus, Star, LoaderCircle, ListVideo } from "@lucide/svelte";
  import { api, type LinkInfo } from "../api";
  import { store } from "../store.svelte";
  import { folderName } from "../format";

  let info = $state<LinkInfo | null>(null);
  let error = $state<string | null>(null);
  let folder = $state<string | null>(null);
  let wholePlaylist = $state(true);
  let extraFolder = $state<string | null>(null);

  const link = $derived(store.pendingLink);
  const favs = $derived(store.settings?.downloadFolders ?? []);

  $effect(() => {
    if (!link) return;
    info = null;
    error = null;
    extraFolder = null;
    folder = store.settings?.defaultDownloadFolder ?? favs[0] ?? null;
    const url = link;
    api.ytInspect(url).then(
      (i) => {
        if (store.pendingLink === url) info = i;
      },
      (e) => {
        if (store.pendingLink === url) error = String(e);
      },
    );
  });

  function close() {
    store.pendingLink = null;
  }

  async function pickFolder() {
    const dir = await open({ directory: true, title: "Elegí dónde guardar" });
    if (typeof dir === "string") {
      extraFolder = dir.replaceAll("\\", "/");
      folder = extraFolder;
    }
  }

  async function addFav() {
    if (!extraFolder) return;
    const f = extraFolder;
    await store.saveSettings((s) => {
      if (!s.downloadFolders.includes(f)) s.downloadFolders = [...s.downloadFolders, f];
      s.defaultDownloadFolder ??= f;
    });
    extraFolder = null;
    store.toast("Carpeta agregada a favoritas");
  }

  async function download() {
    if (!link || !folder) return;
    const playlist = !!info?.isPlaylist && wholePlaylist;
    await api.ytDownload(link, info?.title ?? link, info?.thumb ?? null, folder, playlist);
    if (store.settings?.defaultDownloadFolder !== folder && favs.includes(folder)) {
      store.saveSettings((s) => (s.defaultDownloadFolder = folder));
    }
    store.toast(playlist ? `Bajando ${info?.count} canciones…` : "Bajando…");
    close();
  }

  function listen() {
    if (!info) return;
    store.play(info.songs, 0, info.isPlaylist ? info.title : "YouTube");
    close();
  }
</script>

<svelte:window onkeydown={(e) => link && e.key === "Escape" && close()} />

{#if link}
  <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
  <div class="backdrop" onclick={close}>
    <div class="dialog" tabindex="-1" onclick={(e) => e.stopPropagation()} role="dialog" aria-modal="true" aria-label="Link">
      <button class="icon-btn x" title="Cerrar" onclick={close}><X size={18} /></button>

      <div class="preview">
        {#if info?.thumb}
          <img src={info.thumb} alt="" />
        {:else}
          <div class="ph">{#if !error}<LoaderCircle size={22} class="spin" />{/if}</div>
        {/if}
        <div class="meta">
          {#if info}
            <div class="title">{info.title}</div>
            <div class="muted">
              {#if info.isPlaylist}<ListVideo size={14} /> Playlist · {info.count} videos{:else}{info.songs[0]?.artist}{/if}
            </div>
          {:else if error}
            <div class="title">No se pudo abrir el link</div>
            <div class="err">{error}</div>
          {:else}
            <div class="title muted">Leyendo el link…</div>
            <div class="muted ellipsis small">{link}</div>
          {/if}
        </div>
      </div>

      {#if info?.isPlaylist}
        <label class="check">
          <input type="checkbox" bind:checked={wholePlaylist} />
          Bajar toda la playlist (en una subcarpeta con su nombre)
        </label>
      {/if}

      <div class="label">Guardar en</div>
      <div class="folders">
        {#each favs as f}
          <button class="chip" class:sel={folder === f} title={f} onclick={() => (folder = f)}>
            <Star size={13} fill={folder === f ? "currentColor" : "none"} />
            {folderName(f)}
          </button>
        {/each}
        {#if extraFolder}
          <button class="chip sel" title={extraFolder}>{folderName(extraFolder)}</button>
          <button class="chip ghost" onclick={addFav}><Star size={13} /> Guardar como favorita</button>
        {/if}
        <button class="chip ghost" onclick={pickFolder}><FolderPlus size={14} /> Otra carpeta…</button>
      </div>

      <div class="actions">
        <button class="pill" onclick={listen} disabled={!info}><Play size={16} fill="currentColor" /> Escuchar</button>
        <button class="pill primary" onclick={download} disabled={!info || !folder}><Download size={16} /> Bajar MP3</button>
      </div>
    </div>
  </div>
{/if}

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    z-index: 80;
    display: grid;
    place-items: center;
    background: rgba(0, 0, 0, 0.35);
    animation: fade 0.15s;
  }
  @keyframes fade {
    from {
      opacity: 0;
    }
  }
  .dialog {
    position: relative;
    width: min(560px, calc(100vw - 40px));
    padding: 24px;
    border-radius: 20px;
    background: rgba(26, 30, 35, 0.9);
    backdrop-filter: blur(30px) saturate(140%);
    border: 1px solid var(--line-strong);
    box-shadow: 0 30px 80px rgba(0, 0, 0, 0.5);
    animation: up 0.18s ease-out;
  }
  @keyframes up {
    from {
      transform: translateY(8px) scale(0.98);
      opacity: 0;
    }
  }
  .x {
    position: absolute;
    top: 12px;
    right: 12px;
  }
  .preview {
    display: flex;
    gap: 16px;
    align-items: center;
    padding-right: 30px;
  }
  .preview img,
  .ph {
    width: 136px;
    aspect-ratio: 16/9;
    object-fit: cover;
    border-radius: 10px;
    flex: none;
    background: rgba(255, 255, 255, 0.06);
    display: grid;
    place-items: center;
    color: var(--text-3);
  }
  .meta {
    min-width: 0;
  }
  .title {
    font-weight: 600;
    font-size: 15.5px;
    line-height: 1.3;
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }
  .muted {
    display: flex;
    align-items: center;
    gap: 6px;
    margin-top: 4px;
    font-size: 13px;
  }
  .small {
    font-size: 12px;
  }
  .err {
    color: #f38ba8;
    font-size: 13px;
    margin-top: 4px;
  }
  .check {
    display: flex;
    align-items: center;
    gap: 10px;
    margin-top: 18px;
    font-size: 13.5px;
    color: var(--text-2);
  }
  .check input {
    accent-color: var(--accent);
    width: 16px;
    height: 16px;
  }
  .label {
    margin: 20px 0 8px;
    font-size: 12px;
    font-weight: 600;
    color: var(--text-3);
    text-transform: uppercase;
    letter-spacing: 0.06em;
  }
  .folders {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
  }
  .chip {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    height: 32px;
    padding: 0 12px;
    border-radius: 999px;
    font-size: 13px;
    font-weight: 520;
    background: rgba(255, 255, 255, 0.06);
    border: 1px solid var(--line);
    max-width: 220px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .chip:hover {
    background: rgba(255, 255, 255, 0.1);
  }
  .chip.sel {
    background: color-mix(in srgb, var(--accent) 18%, transparent);
    border-color: color-mix(in srgb, var(--accent) 55%, transparent);
    color: var(--accent);
  }
  .chip.ghost {
    color: var(--text-2);
    border-style: dashed;
  }
  .actions {
    display: flex;
    justify-content: flex-end;
    gap: 10px;
    margin-top: 24px;
  }
</style>
