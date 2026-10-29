<script lang="ts">
  // Barra de tiempo para el dedo: arrastrar previsualiza y recien al soltar se salta.
  import { time } from "@haze/ui/format";
  import { api } from "../api";
  import { store } from "../store.svelte";

  let track: HTMLDivElement;
  let dragging = $state(false);
  let preview = $state(0);

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
    if (dragging) preview = at(e);
  }
  function up() {
    if (!dragging) return;
    dragging = false;
    api.control({ type: "seek", value: preview });
  }
</script>

<div class="seek">
  <div
    class="track"
    class:dragging
    bind:this={track}
    onpointerdown={down}
    onpointermove={move}
    onpointerup={up}
    onpointercancel={() => (dragging = false)}
    role="slider"
    aria-label="Posición"
    aria-valuemin={0}
    aria-valuemax={Math.round(duration)}
    aria-valuenow={Math.round(pos)}
    tabindex="-1"
  >
    <div class="rail"><div class="fill" style:width="{frac * 100}%"></div></div>
    <div class="knob" style:left="{frac * 100}%"></div>
  </div>
  <div class="times">
    <span>{time(pos)}</span>
    <span>{time(duration)}</span>
  </div>
</div>

<style>
  .track {
    position: relative;
    height: 32px;
    display: flex;
    align-items: center;
    touch-action: none;
  }
  .rail {
    position: relative;
    width: 100%;
    height: 4px;
    border-radius: calc(4px * var(--rs));
    background: rgb(var(--ov) / 0.16);
    overflow: hidden;
  }
  .fill {
    height: 100%;
    background: var(--text);
  }
  .knob {
    position: absolute;
    top: 50%;
    width: 14px;
    height: 14px;
    border-radius: 50%;
    background: var(--text);
    transform: translate(-50%, -50%);
    transition: transform 0.12s;
  }
  .dragging .knob {
    transform: translate(-50%, -50%) scale(1.35);
  }
  .times {
    display: flex;
    justify-content: space-between;
    font-size: 12.5px;
    color: var(--text-2);
    font-variant-numeric: tabular-nums;
  }
</style>
