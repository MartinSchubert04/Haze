<script lang="ts">
  // Barra de tiempo: fina, se agranda al pasar el mouse. Arrastrar previsualiza y
  // recien al soltar se salta, asi mpv no recibe cien seeks.
  import { api } from "../api";
  import { store } from "../store.svelte";
  import { time } from "../format";

  let { showTimes = true, thin = false }: { showTimes?: boolean; thin?: boolean } = $props();

  let track: HTMLDivElement;
  let dragging = $state(false);
  let preview = $state(0);
  let hover = $state<number | null>(null);

  const duration = $derived(store.player.duration);
  const pos = $derived(dragging ? preview : store.position);
  const frac = $derived(duration > 0 ? Math.min(1, pos / duration) : 0);

  function at(e: PointerEvent) {
    const r = track.getBoundingClientRect();
    return Math.max(0, Math.min(1, (e.clientX - r.left) / r.width)) * duration;
  }

  function down(e: PointerEvent) {
    if (!duration) return;
    dragging = true;
    preview = at(e);
    track.setPointerCapture(e.pointerId);
  }
  function move(e: PointerEvent) {
    hover = duration ? at(e) : null;
    if (dragging) preview = at(e);
  }
  function up() {
    if (!dragging) return;
    dragging = false;
    api.control({ type: "seek", value: preview });
  }
</script>

<div class="seek" class:thin>
  {#if showTimes}<span class="t">{time(pos)}</span>{/if}
  <div
    class="track"
    class:dragging
    bind:this={track}
    onpointerdown={down}
    onpointermove={move}
    onpointerup={up}
    onpointerleave={() => (hover = null)}
    role="slider"
    aria-label="Posición"
    aria-valuemin={0}
    aria-valuemax={duration}
    aria-valuenow={pos}
    tabindex="-1"
  >
    <div class="rail">
      <div class="fill" class:anim={!dragging} style:width="{frac * 100}%"></div>
    </div>
    <div class="knob" class:anim={!dragging} style:left="{frac * 100}%"></div>
    {#if hover !== null && !dragging}
      <div class="tip" style:left="{(hover / duration) * 100}%">{time(hover)}</div>
    {/if}
  </div>
  {#if showTimes}<span class="t">{time(duration)}</span>{/if}
</div>

<style>
  .seek {
    display: flex;
    align-items: center;
    gap: 10px;
    width: 100%;
  }
  .t {
    font-size: 11.5px;
    color: var(--text-3);
    font-variant-numeric: tabular-nums;
    min-width: 36px;
    text-align: center;
  }
  .track {
    position: relative;
    flex: 1;
    height: 18px;
    display: flex;
    align-items: center;
    cursor: pointer;
    touch-action: none;
  }
  .rail {
    position: relative;
    width: 100%;
    height: 4px;
    border-radius: 4px;
    background: rgba(255, 255, 255, 0.14);
    overflow: hidden;
    transition: height 0.15s;
  }
  .thin .rail {
    height: 3px;
  }
  .track:hover .rail,
  .dragging .rail {
    height: 6px;
  }
  .fill {
    height: 100%;
    background: var(--text);
    border-radius: inherit;
  }
  .track:hover .fill,
  .dragging .fill {
    background: var(--accent);
  }
  .anim {
    transition: width 0.25s linear, left 0.25s linear;
  }
  .knob {
    position: absolute;
    top: 50%;
    width: 12px;
    height: 12px;
    margin: -6px 0 0 -6px;
    border-radius: 50%;
    background: #fff;
    box-shadow: 0 1px 4px rgba(0, 0, 0, 0.4);
    opacity: 0;
    transition: opacity 0.15s;
  }
  .track:hover .knob,
  .dragging .knob {
    opacity: 1;
  }
  .tip {
    position: absolute;
    bottom: 20px;
    transform: translateX(-50%);
    padding: 3px 7px;
    border-radius: 6px;
    background: var(--glass-strong);
    font-size: 11px;
    font-variant-numeric: tabular-nums;
    pointer-events: none;
    white-space: nowrap;
  }
</style>
