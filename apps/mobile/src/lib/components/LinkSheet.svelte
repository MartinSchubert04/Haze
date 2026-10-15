<script lang="ts">
  // Un link pegado en YouTube: se mira que hay detras (video o playlist) y se ofrece
  // escucharlo sin bajar o bajarlo a Music/Haze.
  import { Play, Download, LoaderCircle } from "@lucide/svelte";
  import { plural } from "@haze/ui/format";
  import { api, type LinkInfo } from "../api";
  import { store } from "../store.svelte";

  let { url }: { url: string } = $props();

  let info = $state<LinkInfo | null>(null);
  let error = $state<string | null>(null);

  $effect(() => {
    const u = url;
    info = null;
    error = null;
    api.ytInspect(u).then(
      (i) => (info = i),
      (e) => (error = String(e)),
    );
  });

  function listen() {
    if (!info) return;
    store.close();
    store.play(info.songs, 0, info.isPlaylist ? info.title : "YouTube");
  }

  function download() {
    if (!info) return;
    store.close();
    store.download(url, info.title, info.thumb, info.isPlaylist);
  }
</script>

<div class="backdrop" role="presentation" onclick={() => store.close()}></div>
<div class="sheet" role="dialog" aria-label="Link">
  <div class="grip"></div>
  {#if error}
    <div class="msg error">{error}</div>
  {:else if !info}
    <div class="msg"><LoaderCircle size={18} class="spin" /> Mirando el link…</div>
  {:else}
    <div class="head">
      {#if info.thumb}<img src={info.thumb} alt="" />{/if}
      <div class="text">
        <div class="title">{info.title}</div>
        <div class="sub">{info.isPlaylist ? `Playlist · ${plural(info.count, "canción", "canciones")}` : "Video"}</div>
      </div>
    </div>
    <button class="opt press" onclick={listen}><Play size={20} /> Escuchar sin bajar</button>
    <button class="opt press" onclick={download}>
      <Download size={20} /> {info.isPlaylist ? "Bajar la playlist como MP3" : "Bajar como MP3"}
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
    left: 0;
    right: 0;
    bottom: 0;
    z-index: 61;
    padding: 8px 8px calc(var(--safe-bottom) + 12px);
    border-radius: 20px 20px 0 0;
    background: #1b2025;
    border-top: 1px solid var(--line-strong);
    animation: up 0.22s cubic-bezier(0.2, 0.8, 0.2, 1);
  }
  .grip {
    width: 36px;
    height: 4px;
    border-radius: 4px;
    background: var(--line-strong);
    margin: 0 auto 10px;
  }
  .msg {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 18px 12px 24px;
    color: var(--text-2);
  }
  .msg.error {
    color: #f5c2d0;
  }
  .head {
    display: flex;
    align-items: center;
    gap: 14px;
    padding: 6px 12px 14px;
    border-bottom: 1px solid var(--line);
    margin-bottom: 6px;
  }
  .head img {
    width: 88px;
    aspect-ratio: 16 / 9;
    object-fit: cover;
    border-radius: 8px;
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
    font-size: 13px;
    color: var(--text-2);
  }
  .opt {
    width: 100%;
    display: flex;
    align-items: center;
    gap: 16px;
    padding: 14px 12px;
    border-radius: 12px;
    text-align: left;
  }
  @keyframes up {
    from {
      transform: translateY(100%);
    }
  }
</style>
