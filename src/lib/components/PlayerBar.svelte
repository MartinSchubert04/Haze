<script lang="ts">
  import { Shuffle, SkipBack, SkipForward, Repeat, Repeat1, ListMusic, PictureInPicture2, Maximize2 } from "@lucide/svelte";
  import { api } from "../api";
  import { store } from "../store.svelte";
  import Cover from "./Cover.svelte";
  import PlayButton from "./PlayButton.svelte";
  import Seek from "./Seek.svelte";
  import Volume from "./Volume.svelte";

  const p = $derived(store.player);
  const nextRepeat = $derived(p.repeat === "off" ? "all" : p.repeat === "all" ? "one" : "off");
  const repeatLabel = $derived(p.repeat === "off" ? "Repetir: no" : p.repeat === "all" ? "Repetir: todo" : "Repetir: una");
</script>

<footer class="glass">
  <div class="now">
    {#if p.song}
      <button class="cover-btn" title="Pantalla de reproducción" onclick={() => (store.nowPlayingOpen = true)}>
        <Cover song={p.song} size={52} radius={10} />
        <span class="expand"><Maximize2 size={16} /></span>
      </button>
      <div class="meta">
        <div class="title ellipsis" title={p.song.title}>{p.song.title}</div>
        <div class="artist ellipsis">{p.song.artist || p.context}</div>
      </div>
    {:else}
      <div class="meta"><div class="artist">Elegí algo para escuchar</div></div>
    {/if}
  </div>

  <div class="center">
    <div class="controls">
      <button class="icon-btn" class:on={p.shuffle} title="Aleatorio" onclick={() => api.control({ type: "shuffle", value: !p.shuffle })}>
        <Shuffle size={17} />
      </button>
      <button class="icon-btn" title="Anterior" onclick={() => api.control({ type: "previous" })}>
        <SkipBack size={19} fill="currentColor" />
      </button>
      <PlayButton size={44} />
      <button class="icon-btn" title="Siguiente" onclick={() => api.control({ type: "next" })}>
        <SkipForward size={19} fill="currentColor" />
      </button>
      <button class="icon-btn" class:on={p.repeat !== "off"} title={repeatLabel} onclick={() => api.control({ type: "repeat", value: nextRepeat })}>
        {#if p.repeat === "one"}<Repeat1 size={17} />{:else}<Repeat size={17} />{/if}
      </button>
    </div>
    <Seek />
  </div>

  <div class="right">
    <button class="icon-btn" class:on={store.view.name === "queue"} title="Cola" onclick={() => store.go({ name: "queue" })}>
      <ListMusic size={18} />
    </button>
    <button class="icon-btn" title="Mini reproductor" onclick={() => api.openMini()}>
      <PictureInPicture2 size={17} />
    </button>
    <Volume />
  </div>
</footer>

<style>
  footer {
    height: 92px;
    flex: none;
    display: grid;
    grid-template-columns: minmax(180px, 1fr) minmax(320px, 560px) minmax(180px, 1fr);
    align-items: center;
    gap: 20px;
    padding: 0 20px;
    margin: 0 10px 10px;
    border-radius: var(--radius-lg);
  }
  .now {
    display: flex;
    align-items: center;
    gap: 14px;
    min-width: 0;
  }
  .cover-btn {
    position: relative;
    border-radius: 10px;
    overflow: hidden;
    flex: none;
  }
  .expand {
    position: absolute;
    inset: 0;
    display: grid;
    place-items: center;
    background: rgba(0, 0, 0, 0.45);
    opacity: 0;
    transition: opacity 0.15s;
  }
  .cover-btn:hover .expand {
    opacity: 1;
  }
  .meta {
    min-width: 0;
  }
  .title {
    font-weight: 600;
    font-size: 14.5px;
  }
  .artist {
    color: var(--text-2);
    font-size: 13px;
    margin-top: 2px;
  }
  .center {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 4px;
  }
  .controls {
    display: flex;
    align-items: center;
    gap: 14px;
  }
  .right {
    display: flex;
    align-items: center;
    justify-content: flex-end;
    gap: 4px;
  }
</style>
