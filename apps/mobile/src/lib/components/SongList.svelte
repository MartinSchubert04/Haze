<script lang="ts">
  // Lista de canciones que ocupa toda la pantalla. Solo dibuja las filas que se ven:
  // una biblioteca de miles de temas scrollea igual de liviano.
  import type { Snippet } from "svelte";
  import { time } from "@haze/ui/format";
  import type { Song } from "../api";
  import { store } from "../store.svelte";
  import { longpress } from "../longpress";
  import Cover from "./Cover.svelte";

  let { songs, context, header }: { songs: Song[]; context: string; header?: Snippet } = $props();

  const ROW = 64;
  const EXTRA = 8;

  let scroller: HTMLDivElement;
  let top = $state(0);
  let height = $state(800);
  let headerHeight = $state(0);

  const first = $derived(Math.max(0, Math.floor((top - headerHeight) / ROW) - EXTRA));
  const last = $derived(Math.min(songs.length, Math.ceil((top - headerHeight + height) / ROW) + EXTRA));
  const visible = $derived(songs.slice(first, last));
  const current = $derived(store.player.song?.id);
</script>

<div class="scroller" bind:this={scroller} bind:clientHeight={height} onscroll={() => (top = scroller.scrollTop)}>
  <div bind:clientHeight={headerHeight}>{@render header?.()}</div>
  <div class="rows" style:height="{songs.length * ROW}px">
    {#each visible as song, k (song.id)}
      <button
        class="row press"
        class:current={song.id === current}
        style:top="{(first + k) * ROW}px"
        onclick={() => store.play(songs, first + k, context)}
        use:longpress={() => store.open({ kind: "song", song })}
      >
        <Cover {song} size={46} radius={8} />
        <div class="text">
          <div class="title ellipsis">{song.title}</div>
          <div class="sub ellipsis">{song.artist || "Artista desconocido"}</div>
        </div>
        <div class="dur">{time(song.durationMs / 1000)}</div>
      </button>
    {/each}
  </div>
  <div class="spacer-bottom"></div>
</div>

<style>
  .scroller {
    height: 100%;
    overflow-y: auto;
    overscroll-behavior: contain;
  }
  .rows {
    position: relative;
  }
  .row {
    position: absolute;
    left: 0;
    right: 0;
    height: 64px;
    display: flex;
    align-items: center;
    gap: 14px;
    padding: 0 16px;
    text-align: left;
  }
  .text {
    flex: 1;
    min-width: 0;
  }
  .title {
    font-weight: 550;
  }
  .current .title {
    color: var(--accent);
  }
  .sub {
    font-size: 13px;
    color: var(--text-2);
    margin-top: 2px;
  }
  .dur {
    font-size: 12.5px;
    color: var(--text-3);
    font-variant-numeric: tabular-nums;
  }
</style>
