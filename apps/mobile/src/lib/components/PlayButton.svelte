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
    transition: transform 0.12s;
  }
  .play:active {
    transform: scale(0.92);
  }
  .play.solid {
    background: var(--text);
    color: #111;
  }
  .play:disabled {
    opacity: 0.4;
  }
</style>
