<script lang="ts">
  import { FolderOpen, Play, Link2, CircleCheck, CircleAlert, Clock, LoaderCircle } from "@lucide/svelte";
  import { openPath } from "@tauri-apps/plugin-opener";
  import { api } from "../api";
  import { store } from "../store.svelte";
  import { folderName } from "../format";

  const finished = $derived(store.downloads.filter((d) => d.status === "done" || d.status === "error").length);

  function playFiles(files: string[]) {
    const songs = files.map((f) => store.songs.find((s) => s.path.toLowerCase() === f.toLowerCase())).filter((s) => !!s);
    if (songs.length) store.play(songs, 0, "Descargas");
    else store.toast("Todavía no está en la biblioteca", "error");
  }

  async function paste() {
    try {
      const t = (await navigator.clipboard.readText()).trim();
      if (/^https?:\/\//.test(t)) store.pendingLink = t;
      else store.toast("No hay un link en el portapapeles", "error");
    } catch {
      store.toast("No se pudo leer el portapapeles", "error");
    }
  }
</script>

<div class="scroll">
  <div class="page">
    <div class="top">
      <div>
        <h1 class="page-title">Descargas</h1>
        <div class="page-sub">Lo que bajás de YouTube queda como MP3 con título, artista y tapa.</div>
      </div>
      <div class="acts">
        {#if finished}<button class="pill" onclick={() => api.clearDownloads()}>Limpiar terminadas</button>{/if}
        <button class="pill primary" onclick={paste}><Link2 size={16} /> Pegar link</button>
      </div>
    </div>

    {#if store.toolProgress}
      <div class="tool glass">
        <LoaderCircle size={16} class="spin" />
        Preparando {store.toolProgress.tool}… {Math.round(store.toolProgress.progress * 100)}%
      </div>
    {/if}

    {#if store.downloads.length === 0}
      <div class="empty">
        <h3>No bajaste nada todavía</h3>
        <p>Copiá un link de YouTube y pegalo con <b>Ctrl V</b> en cualquier parte de la app.</p>
      </div>
    {:else}
      <div class="list">
        {#each store.downloads as d (d.id)}
          <div class="job">
            {#if d.thumb}<img src={d.thumb} alt="" />{:else}<div class="ph"></div>{/if}
            <div class="info">
              <div class="t ellipsis">{d.title}</div>
              <div class="s">
                {#if d.status === "queued"}<Clock size={13} /> En espera
                {:else if d.status === "downloading"}<LoaderCircle size={13} class="spin" /> Bajando {Math.round(d.progress * 100)}%{d.item ? ` · ${d.item}` : ""}
                {:else if d.status === "converting"}<LoaderCircle size={13} class="spin" /> Pasando a MP3{d.item ? ` · ${d.item}` : ""}
                {:else if d.status === "done"}<CircleCheck size={13} color="#82c896" /> Listo{d.files.length > 1 ? ` · ${d.files.length} canciones` : ""} · {folderName(d.folder)}
                {:else}<CircleAlert size={13} color="#f38ba8" /> {d.error}{/if}
              </div>
              {#if d.status === "done" && d.error}<div class="warn ellipsis">Algunos videos fallaron: {d.error}</div>{/if}
              {#if d.status === "downloading" || d.status === "converting"}
                <div class="bar"><div class="fill" class:conv={d.status === "converting"} style:width="{d.progress * 100}%"></div></div>
              {/if}
            </div>
            <div class="btns">
              {#if d.status === "done" && d.files.length}
                <button class="icon-btn" title="Reproducir" onclick={() => playFiles(d.files)}><Play size={16} fill="currentColor" /></button>
              {/if}
              <button class="icon-btn" title="Abrir carpeta" onclick={() => openPath(d.folder.replaceAll("/", "\\"))}><FolderOpen size={16} /></button>
            </div>
          </div>
        {/each}
      </div>
    {/if}
  </div>
</div>

<style>
  .scroll {
    height: 100%;
    overflow-y: auto;
  }
  .top {
    display: flex;
    align-items: flex-end;
    justify-content: space-between;
    gap: 20px;
  }
  .acts {
    display: flex;
    gap: 10px;
    margin-bottom: 18px;
  }
  .tool {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 12px 16px;
    border-radius: var(--radius);
    margin-bottom: 14px;
    font-size: 13px;
  }
  .list {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .job {
    display: grid;
    grid-template-columns: 96px minmax(0, 1fr) auto;
    gap: 14px;
    align-items: center;
    padding: 10px;
    border-radius: var(--radius-lg);
    background: var(--tile);
  }
  .job img,
  .ph {
    width: 96px;
    aspect-ratio: 16/9;
    object-fit: cover;
    border-radius: 8px;
    background: rgba(255, 255, 255, 0.06);
  }
  .t {
    font-weight: 560;
  }
  .s {
    display: flex;
    align-items: center;
    gap: 6px;
    color: var(--text-2);
    font-size: 12.5px;
    margin-top: 4px;
  }
  .warn {
    color: #fab387;
    font-size: 12px;
    margin-top: 3px;
  }
  .bar {
    height: 4px;
    border-radius: 4px;
    background: rgba(255, 255, 255, 0.1);
    margin-top: 8px;
    overflow: hidden;
  }
  .fill {
    height: 100%;
    background: var(--accent);
    transition: width 0.3s;
  }
  .fill.conv {
    width: 100% !important;
    animation: shimmer 1.2s ease-in-out infinite;
  }
  @keyframes shimmer {
    50% {
      opacity: 0.4;
    }
  }
  .btns {
    display: flex;
    gap: 2px;
  }
</style>
