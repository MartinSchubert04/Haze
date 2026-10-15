<script lang="ts">
  import { Play, Pause, LoaderCircle } from "@lucide/svelte";
  import { api } from "../api";
  import { store } from "../store.svelte";

  let { size = 44, ring = true, solid = false }: { size?: number; ring?: boolean; solid?: boolean } = $props();

  const stroke = $derived(Math.max(2, size / 18));
  const r = $derived(size / 2 - stroke / 2);
  const c = $derived(2 * Math.PI * r);
  const frac = $derived(store.player.duration > 0 ? Math.min(1, store.position / store.player.duration) : 0);
</script>

<button
  class="play"
  class:solid
  style:width="{size}px"
  style:height="{size}px"
  title={store.player.playing ? "Pausar (Espacio)" : "Reproducir (Espacio)"}
  disabled={!store.player.song}
  onclick={() => api.control({ type: "toggle" })}
>
  {#if ring}
    <svg width={size} height={size} viewBox="0 0 {size} {size}" aria-hidden="true">
      <circle cx={size / 2} cy={size / 2} {r} fill="none" stroke="rgba(255,255,255,.14)" stroke-width={stroke} />
      <circle
        cx={size / 2}
        cy={size / 2}
        {r}
        fill="none"
        stroke="var(--accent)"
        stroke-width={stroke}
        stroke-linecap="round"
        stroke-dasharray={c}
        stroke-dashoffset={c * (1 - frac)}
        transform="rotate(-90 {size / 2} {size / 2})"
      />
    </svg>
  {/if}
  <span class="glyph">
    {#if store.player.buffering}
      <LoaderCircle size={size * 0.42} class="spin" />
    {:else if store.player.playing}
      <Pause size={size * 0.4} fill="currentColor" strokeWidth={0} />
    {:else}
      <Play size={size * 0.4} fill="currentColor" strokeWidth={0} style="margin-left:{size * 0.05}px" />
    {/if}
  </span>
</button>

<style>
  .play {
    position: relative;
    display: grid;
    place-items: center;
    border-radius: 50%;
    flex: none;
    background: rgba(255, 255, 255, 0.08);
    transition: transform 0.12s, background 0.15s;
  }
  .play:hover {
    background: rgba(255, 255, 255, 0.14);
  }
  .play:active {
    transform: scale(0.94);
  }
  .play.solid {
    background: var(--text);
    color: #111;
  }
  .play.solid:hover {
    background: #fff;
  }
  .play:disabled {
    opacity: 0.4;
    pointer-events: none;
  }
  svg {
    position: absolute;
    inset: 0;
  }
  svg circle:last-child {
    transition: stroke-dashoffset 0.25s linear;
  }
  .glyph {
    display: grid;
    place-items: center;
  }
</style>
