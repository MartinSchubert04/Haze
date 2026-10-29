<script lang="ts">
  // Un link de YouTube (pegado o de una busqueda), como el dialogo de escritorio: primero
  // se lee el link (y yt-dlp queda listo), recien ahi se puede escuchar o bajar. Al bajar
  // la hoja sigue abierta con el avance, y al final dice si quedo o por que fallo.
  import { Play, Download, LoaderCircle, Check, CircleAlert, ListVideo, RotateCcw } from "@lucide/svelte";
  import { plural } from "@haze/ui/format";
  import { api, type LinkInfo, type Song } from "../api";
  import { store } from "../store.svelte";

  let { url, preview }: { url: string; preview?: Song } = $props();

  let info = $state<LinkInfo | null>(null);
  let error = $state<string | null>(null);
  let wholePlaylist = $state(true);
  let jobId = $state<number | null>(null);
  let starting = $state(false);

  const job = $derived(jobId === null ? null : (store.downloads.find((d) => d.id === jobId) ?? null));
  const title = $derived(info?.title ?? preview?.title ?? "");
  const thumb = $derived(info?.thumb ?? preview?.thumb ?? null);

  function inspect() {
    const u = url;
    info = null;
    error = null;
    api.ytInspect(u).then(
      (i) => u === url && (info = i),
      (e) => u === url && (error = String(e)),
    );
  }

  $effect(() => {
    url;
    jobId = null;
    inspect();
  });

  function listen() {
    if (!info) return;
    store.close();
    store.play(info.songs, 0, info.isPlaylist ? info.title : "YouTube");
  }

  async function download() {
    if (!info || starting) return;
    starting = true;
    const playlist = info.isPlaylist && wholePlaylist;
    jobId = await store.download(url, info.title, info.thumb, playlist);
    starting = false;
  }

  function label(status: string, progress: number, item: string | null) {
    const base =
      status === "queued"
        ? "En espera…"
        : status === "converting"
          ? "Pasando a MP3…"
          : `Bajando ${Math.round(progress * 100)}%`;
    return item ? `${base} · ${item}` : base;
  }
</script>

<div class="backdrop" role="presentation" onclick={() => store.close()}></div>
<div class="sheet" role="dialog" aria-label="Link">
  <div class="grip"></div>

  <div class="head">
    {#if thumb}
      <img src={thumb} alt="" />
    {:else}
      <div class="ph">{#if !error}<LoaderCircle size={20} class="spin" />{/if}</div>
    {/if}
    <div class="text">
      {#if title}<div class="title">{title}</div>{/if}
      {#if info}
        <div class="sub">
          {#if info.isPlaylist}<ListVideo size={13} /> Playlist · {plural(info.count, "canción", "canciones")}
          {:else}{info.songs[0]?.artist || "Video"}{/if}
        </div>
      {:else if error}
        <div class="sub">No se pudo abrir el link</div>
      {:else}
        <div class="sub loading"><LoaderCircle size={13} class="spin" /> Leyendo el link…</div>
      {/if}
    </div>
  </div>

  {#if error}
    <div class="msg error">{error}</div>
    <button class="opt press" onclick={inspect}><RotateCcw size={20} /> Probar de nuevo</button>
  {:else if job}
    <!-- Ya se pidio la descarga: el avance de esa descarga, en vivo. -->
    <div class="status">
      {#if job.status === "done"}
        <div class="line ok"><Check size={18} /> Listo: está en Music/Haze</div>
        {#if job.error}<div class="hint">Algunos videos fallaron: {job.error}</div>{/if}
      {:else if job.status === "error"}
        <div class="line bad"><CircleAlert size={18} /> No se pudo bajar</div>
        <div class="msg error">{job.error}</div>
      {:else}
        <div class="line">{label(job.status, job.progress, job.item)}</div>
        <div class="bar" class:busy={job.status !== "downloading" || job.progress === 0}>
          <div style:width="{Math.round(job.progress * 100)}%"></div>
        </div>
        <div class="hint">Podés cerrar esto: sigue bajando aunque salgas de la app.</div>
      {/if}
    </div>
    {#if job.status === "error"}
      <button class="opt press" onclick={download}><RotateCcw size={20} /> Reintentar</button>
    {:else}
      <button class="opt press" onclick={() => store.close()}><Check size={20} /> Cerrar</button>
    {/if}
  {:else}
    {#if info?.isPlaylist}
      <label class="check">
        <input type="checkbox" bind:checked={wholePlaylist} />
        Bajar toda la playlist (en una subcarpeta con su nombre)
      </label>
    {/if}
    <button class="opt press" disabled={!info} onclick={listen}><Play size={20} /> Escuchar sin bajar</button>
    <button class="opt press primary" disabled={!info || starting} onclick={download}>
      {#if starting}<LoaderCircle size={20} class="spin" />{:else}<Download size={20} />{/if}
      {info?.isPlaylist && wholePlaylist ? "Bajar la playlist como MP3" : "Bajar como MP3"}
    </button>
  {/if}
</div>

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    z-index: 60;
    background: rgba(0, 0, 0, 0.5);
  }
  .sheet {
    position: fixed;
    /* --pop puede ser translucido: debajo va el fondo del tema, asi no se ven las
       pestañas a traves. */
    background-color: var(--bg);
    left: 0;
    right: 0;
    bottom: 0;
    z-index: 61;
    padding: 8px 8px calc(var(--safe-bottom) + 12px);
    border-radius: calc(20px * var(--rs)) calc(20px * var(--rs)) 0 0;
    background-image: linear-gradient(var(--pop), var(--pop));
    border-top: var(--border) solid var(--line-strong);
    animation: up 0.22s cubic-bezier(0.2, 0.8, 0.2, 1);
  }
  .grip {
    width: 36px;
    height: 4px;
    border-radius: calc(4px * var(--rs));
    background: var(--line-strong);
    margin: 0 auto 10px;
  }
  .head {
    display: flex;
    align-items: center;
    gap: 14px;
    padding: 6px 12px 14px;
    border-bottom: 1px solid var(--line);
    margin-bottom: 6px;
  }
  .head img,
  .ph {
    flex: none;
    width: 96px;
    aspect-ratio: 16 / 9;
    object-fit: cover;
    border-radius: calc(8px * var(--rs));
  }
  .ph {
    display: grid;
    place-items: center;
    background: rgb(var(--ov) / 0.06);
    color: var(--text-3);
  }
  .text {
    min-width: 0;
  }
  .title {
    font-weight: 600;
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }
  .sub {
    display: flex;
    align-items: center;
    gap: 6px;
    margin-top: 2px;
    font-size: 13px;
    color: var(--text-2);
  }
  .msg {
    padding: 4px 12px 10px;
    font-size: 14px;
    color: var(--text-2);
    word-break: break-word;
  }
  .msg.error {
    color: color-mix(in srgb, var(--danger) 65%, var(--text));
  }
  .check {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 10px 12px;
    font-size: 14px;
    color: var(--text-2);
  }
  .check input {
    width: 20px;
    height: 20px;
    accent-color: var(--accent);
  }
  .opt {
    width: 100%;
    display: flex;
    align-items: center;
    gap: 16px;
    padding: 14px 12px;
    border-radius: calc(12px * var(--rs));
    text-align: left;
  }
  .opt.primary {
    color: var(--accent);
    font-weight: 600;
  }
  .opt:disabled {
    opacity: 0.4;
  }
  .status {
    padding: 6px 12px 12px;
  }
  .line {
    display: flex;
    align-items: center;
    gap: 10px;
    font-weight: 560;
  }
  .line.ok {
    color: var(--ok);
  }
  .line.bad {
    color: var(--danger);
    margin-bottom: 4px;
  }
  .status .msg.error {
    padding: 0;
  }
  .bar {
    height: 4px;
    margin: 12px 0 10px;
    border-radius: var(--round);
    background: rgb(var(--ov) / 0.1);
    overflow: hidden;
  }
  .bar div {
    height: 100%;
    background: var(--accent);
    transition: width 0.3s;
  }
  /* Sin porcentaje todavia (preparando, convirtiendo): una franja que va y viene. */
  .bar.busy div {
    width: 30% !important;
    animation: busy 1.2s ease-in-out infinite;
  }
  .hint {
    font-size: 13px;
    color: var(--text-3);
  }
  @keyframes busy {
    from {
      transform: translateX(-100%);
    }
    to {
      transform: translateX(340%);
    }
  }
  @keyframes up {
    from {
      transform: translateY(100%);
    }
  }
</style>
