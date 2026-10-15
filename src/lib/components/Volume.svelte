<script lang="ts">
  import { Volume, Volume1, Volume2, VolumeX } from "@lucide/svelte";
  import { api } from "../api";
  import { store } from "../store.svelte";

  let track: HTMLDivElement;
  let dragging = false;
  let local = $state<number | null>(null);
  let muted: number | null = null;

  const vol = $derived(local ?? store.player.volume);

  function at(e: PointerEvent) {
    const r = track.getBoundingClientRect();
    return Math.round(Math.max(0, Math.min(1, (e.clientX - r.left) / r.width)) * 100);
  }
  function set(v: number) {
    local = v;
    api.control({ type: "volume", value: v });
  }
  function down(e: PointerEvent) {
    dragging = true;
    track.setPointerCapture(e.pointerId);
    set(at(e));
  }
  function move(e: PointerEvent) {
    if (dragging) set(at(e));
  }
  function up() {
    dragging = false;
    setTimeout(() => (local = null), 300);
  }
  function wheel(e: WheelEvent) {
    e.preventDefault();
    set(Math.max(0, Math.min(100, vol + (e.deltaY < 0 ? 5 : -5))));
    setTimeout(() => (local = null), 300);
  }
  function toggleMute() {
    if (vol > 0) {
      muted = vol;
      set(0);
    } else {
      set(muted ?? 60);
    }
    setTimeout(() => (local = null), 300);
  }
</script>

<div class="vol" onwheel={wheel}>
  <button class="icon-btn" title="Silenciar" onclick={toggleMute}>
    {#if vol === 0}<VolumeX size={18} />{:else if vol < 34}<Volume size={18} />{:else if vol < 67}<Volume1 size={18} />{:else}<Volume2 size={18} />{/if}
  </button>
  <div class="track" bind:this={track} onpointerdown={down} onpointermove={move} onpointerup={up} role="slider" aria-label="Volumen" aria-valuenow={vol} tabindex="-1">
    <div class="rail"><div class="fill" style:width="{vol}%"></div></div>
  </div>
</div>

<style>
  .vol {
    display: flex;
    align-items: center;
    gap: 4px;
  }
  .track {
    width: 96px;
    height: 18px;
    display: flex;
    align-items: center;
    cursor: pointer;
    touch-action: none;
  }
  .rail {
    width: 100%;
    height: 4px;
    border-radius: 4px;
    background: rgba(255, 255, 255, 0.14);
    overflow: hidden;
    transition: height 0.15s;
  }
  .track:hover .rail {
    height: 6px;
  }
  .fill {
    height: 100%;
    background: var(--text);
  }
  .track:hover .fill {
    background: var(--accent);
  }
</style>
