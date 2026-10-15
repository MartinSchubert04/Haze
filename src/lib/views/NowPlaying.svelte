<script lang="ts">
  import { ChevronDown, Shuffle, SkipBack, SkipForward, Repeat, Repeat1, ListMusic, PictureInPicture2, Download } from "@lucide/svelte";
  import { api, coverUrl, type QueueView } from "../api";
  import { store } from "../store.svelte";
  import { dominantColor } from "../color";
  import Cover from "../components/Cover.svelte";
  import PlayButton from "../components/PlayButton.svelte";
  import Seek from "../components/Seek.svelte";
  import Volume from "../components/Volume.svelte";

  const p = $derived(store.player);
  let tint = $state<string | null>(null);
  let q = $state<QueueView | null>(null);

  $effect(() => {
    const url = coverUrl(p.song);
    if (!store.settings?.appearance.tintFromArtwork) {
      tint = null;
      return;
    }
    dominantColor(url).then((c) => (tint = c));
  });

  $effect(() => {
    p.queueVersion;
    api.queue().then((v) => (q = v));
  });

  const upNext = $derived(q && q.currentIndex !== null ? q.entries.slice(q.currentIndex + 1, q.currentIndex + 13).map((e, k) => ({ e, i: q!.currentIndex! + 1 + k })) : []);
  const nextRepeat = $derived(p.repeat === "off" ? "all" : p.repeat === "all" ? "one" : "off");
</script>

<div class="np" style:--tint={tint ?? "transparent"}>
  <div class="tint"></div>
  <button class="icon-btn close" title="Cerrar (Esc)" onclick={() => (store.nowPlayingOpen = false)}><ChevronDown size={22} /></button>

  <div class="body">
    <div class="art">
      <Cover song={p.song} size="100%" radius={18} />
    </div>

    <div class="side">
      <div class="ctx ellipsis">{p.context || " "}</div>
      <h1 class="title">{p.song?.title ?? "Nada sonando"}</h1>
      <div class="artist ellipsis">{p.song?.artist || ""}</div>

      <div class="seek"><Seek /></div>

      <div class="controls">
        <button class="icon-btn big" class:on={p.shuffle} title="Aleatorio" onclick={() => api.control({ type: "shuffle", value: !p.shuffle })}>
          <Shuffle size={20} />
        </button>
        <button class="icon-btn big" title="Anterior" onclick={() => api.control({ type: "previous" })}><SkipBack size={26} fill="currentColor" /></button>
        <PlayButton size={76} solid />
        <button class="icon-btn big" title="Siguiente" onclick={() => api.control({ type: "next" })}><SkipForward size={26} fill="currentColor" /></button>
        <button class="icon-btn big" class:on={p.repeat !== "off"} title="Repetir" onclick={() => api.control({ type: "repeat", value: nextRepeat })}>
          {#if p.repeat === "one"}<Repeat1 size={20} />{:else}<Repeat size={20} />{/if}
        </button>
      </div>

      <div class="extras">
        <button class="icon-btn" title="Cola" onclick={() => store.go({ name: "queue" })}><ListMusic size={18} /></button>
        <button class="icon-btn" title="Mini reproductor" onclick={() => api.openMini()}><PictureInPicture2 size={17} /></button>
        {#if p.song?.kind === "stream"}
          <button class="icon-btn" title="Bajar como MP3" onclick={() => (store.pendingLink = p.song!.path)}><Download size={17} /></button>
        {/if}
        <div class="vol"><Volume /></div>
      </div>
    </div>
  </div>

  {#if upNext.length}
    <div class="strip">
      <div class="label">A continuación</div>
      <div class="cards">
        {#each upNext as { e, i } (e.uid)}
          <button class="card" title="{e.song.title} — {e.song.artist}" onclick={() => api.queueAction("jump", i)}>
            <Cover song={e.song} size={92} radius={10} />
            <div class="ct ellipsis">{e.song.title}</div>
          </button>
        {/each}
      </div>
    </div>
  {/if}
</div>

<svelte:window onkeydown={(e) => e.key === "Escape" && (store.nowPlayingOpen = false)} />

<style>
  .np {
    position: absolute;
    inset: 0;
    z-index: 20;
    display: flex;
    flex-direction: column;
    background: rgba(10, 13, 16, 0.55);
    backdrop-filter: blur(40px) saturate(150%);
    animation: rise 0.25s cubic-bezier(0.2, 0.8, 0.2, 1);
    overflow: hidden;
  }
  @keyframes rise {
    from {
      transform: translateY(24px);
      opacity: 0;
    }
  }
  .tint {
    position: absolute;
    inset: 0;
    background:
      radial-gradient(ellipse 70% 80% at 25% 40%, color-mix(in srgb, var(--tint) 55%, transparent), transparent 70%),
      radial-gradient(ellipse 60% 60% at 90% 100%, color-mix(in srgb, var(--tint) 25%, transparent), transparent 70%);
    transition: background 0.6s;
    pointer-events: none;
  }
  .close {
    position: absolute;
    top: 8px;
    left: 16px;
    z-index: 2;
  }
  .body {
    position: relative;
    flex: 1;
    min-height: 0;
    display: grid;
    grid-template-columns: minmax(0, auto) minmax(300px, 520px);
    justify-content: center;
    align-items: center;
    gap: clamp(28px, 5vw, 72px);
    padding: 48px 48px 12px;
  }
  .art {
    height: min(52vh, 440px);
    aspect-ratio: 1;
    border-radius: 18px;
    box-shadow: 0 30px 80px rgba(0, 0, 0, 0.45);
  }
  .side {
    min-width: 0;
  }
  .ctx {
    color: var(--text-3);
    font-size: 12.5px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.06em;
  }
  .title {
    font-size: clamp(24px, 3vw, 34px);
    font-weight: 680;
    letter-spacing: -0.02em;
    line-height: 1.15;
    margin: 8px 0 6px;
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }
  .artist {
    color: var(--text-2);
    font-size: 17px;
  }
  .seek {
    margin: 28px 0 16px;
  }
  .controls {
    display: flex;
    align-items: center;
    justify-content: space-between;
    max-width: 420px;
  }
  .big {
    width: 46px;
    height: 46px;
  }
  .extras {
    display: flex;
    align-items: center;
    gap: 4px;
    margin-top: 22px;
  }
  .vol {
    margin-left: auto;
  }
  .strip {
    position: relative;
    padding: 0 48px 18px;
  }
  .label {
    font-size: 12px;
    font-weight: 600;
    color: var(--text-3);
    text-transform: uppercase;
    letter-spacing: 0.06em;
    margin-bottom: 10px;
  }
  .cards {
    display: flex;
    gap: 14px;
    overflow-x: auto;
    padding-bottom: 6px;
  }
  .card {
    width: 92px;
    flex: none;
    text-align: left;
    transition: transform 0.15s;
  }
  .card:hover {
    transform: translateY(-3px);
  }
  .ct {
    font-size: 12px;
    color: var(--text-2);
    margin-top: 6px;
  }
</style>
