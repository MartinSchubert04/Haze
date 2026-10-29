<script lang="ts">
  // "Reproduciendo" a pantalla completa. Se cierra con "atras", la flecha o deslizando
  // hacia abajo.
  import { ChevronDown, Shuffle, SkipBack, SkipForward, Repeat, Repeat1, ListMusic } from "@lucide/svelte";
  import { dominantColor } from "@haze/ui/color";
  import { api, coverUrl } from "../api";
  import { store } from "../store.svelte";
  import Cover from "../components/Cover.svelte";
  import PlayButton from "../components/PlayButton.svelte";
  import Seek from "../components/Seek.svelte";

  const p = $derived(store.player);
  let tint = $state<string | null>(null);

  $effect(() => {
    const url = coverUrl(p.song);
    if (!store.settings?.appearance.tintFromArtwork) {
      tint = null;
      return;
    }
    dominantColor(url).then((c) => (tint = c));
  });

  const nextRepeat = $derived(p.repeat === "off" ? "all" : p.repeat === "all" ? "one" : "off");

  // Deslizar hacia abajo desde la parte de arriba (la tapa) cierra.
  let startY: number | null = null;
  let drag = $state(0);
  function down(e: PointerEvent) {
    startY = e.clientY;
  }
  function move(e: PointerEvent) {
    if (startY !== null) drag = Math.max(0, e.clientY - startY);
  }
  function up() {
    if (drag > 120) store.close();
    startY = null;
    drag = 0;
  }

  function openQueue() {
    store.setTab("queue");
  }
</script>

<div class="np" style:--tint={tint ?? "transparent"} style:transform={drag ? `translateY(${drag}px)` : undefined} class:dragging={drag > 0}>
  <div class="tint"></div>
  <div
    class="grab"
    role="presentation"
    onpointerdown={down}
    onpointermove={move}
    onpointerup={up}
    onpointercancel={up}
  >
    <div class="bar">
      <button class="icon-btn" aria-label="Cerrar" onclick={() => store.close()}><ChevronDown size={26} /></button>
      <div class="ctx ellipsis">{p.context}</div>
      <button class="icon-btn" aria-label="Cola" onclick={openQueue}><ListMusic size={22} /></button>
    </div>
    <div class="art"><Cover song={p.song} size="100%" radius={18} /></div>
  </div>

  <div class="info">
    <h1 class="title">{p.song?.title ?? ""}</h1>
    <div class="artist ellipsis">{p.song?.artist || "Artista desconocido"}</div>
  </div>

  <div class="seek"><Seek /></div>

  <div class="controls">
    <button class="icon-btn" class:on={p.shuffle} aria-label="Aleatorio" onclick={() => api.control({ type: "shuffle", value: !p.shuffle })}>
      <Shuffle size={22} />
    </button>
    <button class="icon-btn big" aria-label="Anterior" onclick={() => api.control({ type: "previous" })}><SkipBack size={30} fill="currentColor" /></button>
    <PlayButton size={76} solid />
    <button class="icon-btn big" aria-label="Siguiente" onclick={() => api.control({ type: "next" })}><SkipForward size={30} fill="currentColor" /></button>
    <button class="icon-btn" class:on={p.repeat !== "off"} aria-label="Repetir" onclick={() => api.control({ type: "repeat", value: nextRepeat })}>
      {#if p.repeat === "one"}<Repeat1 size={22} />{:else}<Repeat size={22} />{/if}
    </button>
  </div>
</div>

<style>
  .np {
    position: fixed;
    inset: 0;
    z-index: 50;
    display: flex;
    flex-direction: column;
    padding: var(--safe-top) 20px calc(var(--safe-bottom) + 24px);
    background: var(--bg);
    animation: rise 0.28s cubic-bezier(0.2, 0.8, 0.2, 1);
    transition: transform 0.2s;
    overflow: hidden;
  }
  .np.dragging {
    transition: none;
  }
  @keyframes rise {
    from {
      transform: translateY(100%);
    }
  }
  .tint {
    position: absolute;
    inset: 0;
    background:
      radial-gradient(ellipse 90% 60% at 50% 25%, color-mix(in srgb, var(--tint) 55%, transparent), transparent 70%),
      radial-gradient(ellipse 80% 50% at 50% 100%, color-mix(in srgb, var(--tint) 22%, transparent), transparent 70%);
    transition: background 0.6s;
    pointer-events: none;
  }
  .grab {
    position: relative;
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
    touch-action: none;
  }
  .bar {
    display: flex;
    align-items: center;
    gap: 8px;
    height: 56px;
    margin: 0 -10px;
  }
  .ctx {
    flex: 1;
    text-align: center;
    font-size: 13px;
    font-weight: 600;
    color: var(--text-2);
  }
  .art {
    flex: 1;
    min-height: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 12px 0 24px;
  }
  .art :global(.cover) {
    width: min(100%, 52vh) !important;
    height: auto !important;
    aspect-ratio: 1;
    box-shadow: 0 24px 60px rgba(0, 0, 0, 0.5);
  }
  .info {
    position: relative;
  }
  .title {
    font-size: 24px;
    font-weight: 700;
    letter-spacing: -0.01em;
    margin: 0;
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }
  .artist {
    margin-top: 4px;
    font-size: 16px;
    color: var(--text-2);
  }
  .seek {
    position: relative;
    margin: 18px 0 8px;
  }
  .controls {
    position: relative;
    display: flex;
    align-items: center;
    justify-content: space-between;
  }
  .icon-btn {
    color: var(--text);
  }
  .icon-btn.big {
    width: 60px;
    height: 60px;
  }
  .icon-btn.on {
    color: var(--accent);
  }
</style>
