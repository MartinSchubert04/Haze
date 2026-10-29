<script lang="ts">
  import { Play, Pause, LoaderCircle } from "@lucide/svelte";
  import { api } from "../api";
  import { store } from "../store.svelte";

  let { size = 44, solid = false }: { size?: number; solid?: boolean } = $props();
</script>

<button
  class="play"
  class:solid
  style:width="{size}px"
  style:height="{size}px"
  aria-label={store.player.playing ? "Pausar" : "Reproducir"}
  disabled={!store.player.song}
  onclick={(e) => {
    e.stopPropagation();
    api.control({ type: "toggle" });
  }}
>
  {#if store.player.buffering}
    <LoaderCircle size={size * 0.42} class="spin" />
  {:else if store.player.playing}
    <Pause size={size * 0.4} fill="currentColor" strokeWidth={0} />
  {:else}
    <Play size={size * 0.4} fill="currentColor" strokeWidth={0} style="margin-left:{size * 0.05}px" />
  {/if}
</button>

<style>
  .play {
    display: grid;
    place-items: center;
    border-radius: 50%;
    flex: none;
    transition:
      transform 0.4s cubic-bezier(0.2, 0.8, 0.2, 1) 0.06s,
      filter var(--release);
  }
  .play:global(.pressed) {
    transform: scale(0.9);
    filter: brightness(0.85);
    transition:
      transform 0.08s,
      filter 0.05s;
  }
  .play.solid {
    background: var(--text);
    color: var(--bg);
  }
  .play:disabled {
    opacity: 0.4;
  }
</style>
