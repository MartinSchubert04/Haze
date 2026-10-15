<script lang="ts">
  // Lista virtualizada: solo se dibujan las filas visibles, asi miles de canciones
  // no pesan. El encabezado de la pagina va adentro del mismo scroll.
  import type { Snippet } from "svelte";
  import { MoreHorizontal, AudioLines, Download } from "@lucide/svelte";
  import type { Song } from "../api";
  import { store } from "../store.svelte";
  import { songMenu } from "../menu.svelte";
  import { time, indexLetter } from "../format";
  import Cover from "./Cover.svelte";

  let {
    songs,
    context,
    header,
    alphabet = false,
    showFolder = true,
    onDownload,
  }: {
    songs: Song[];
    context: string;
    header?: Snippet;
    alphabet?: boolean;
    showFolder?: boolean;
    onDownload?: (s: Song) => void;
  } = $props();

  const ROW = 60;
  let scroller: HTMLDivElement;
  let headerH = $state(0);
  let scrollTop = $state(0);
  let viewH = $state(600);

  const start = $derived(Math.max(0, Math.floor((scrollTop - headerH) / ROW) - 6));
  const end = $derived(Math.min(songs.length, Math.ceil((scrollTop - headerH + viewH) / ROW) + 6));
  const visible = $derived(songs.slice(start, end));

  const letters = $derived.by(() => {
    if (!alphabet) return [];
    const seen = new Map<string, number>();
    songs.forEach((s, i) => {
      const l = indexLetter(s.title);
      if (!seen.has(l)) seen.set(l, i);
    });
    return [...seen.entries()];
  });
  let activeLetter = $state<string | null>(null);

  function jump(i: number, l: string) {
    scroller.scrollTo({ top: headerH + i * ROW - 8 });
    activeLetter = l;
    setTimeout(() => (activeLetter = null), 700);
  }

  function play(i: number) {
    store.play(songs, i, context);
  }

  const date = (s: Song) => {
    if (!s.mtime) return "";
    const d = new Date(s.mtime * 1000);
    return `${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")}`;
  };

  export function scrollToTop() {
    scroller?.scrollTo({ top: 0 });
  }
</script>

<div class="wrap">
  <div class="scroller" bind:this={scroller} bind:clientHeight={viewH} onscroll={() => (scrollTop = scroller.scrollTop)}>
    <div bind:clientHeight={headerH}>{@render header?.()}</div>
    <div class="rows" style:height="{songs.length * ROW}px">
      {#each visible as song, k (song.id + ":" + (start + k))}
        {@const i = start + k}
        {@const current = store.player.song?.id === song.id}
        <div
          class="row"
          class:current
          style:top="{i * ROW}px"
          role="button"
          tabindex="0"
          onclick={() => play(i)}
          onkeydown={(e) => e.key === "Enter" && play(i)}
          oncontextmenu={(e) => songMenu(e, song, () => play(i))}
        >
          <button class="menu-btn" title="Más" onclick={(e) => { e.stopPropagation(); songMenu(e, song, () => play(i)); }}>
            <MoreHorizontal size={16} />
          </button>
          <div class="cov">
            <Cover {song} size={42} radius={8} />
            {#if current}
              <span class="eq" class:paused={!store.player.playing}><AudioLines size={18} /></span>
            {/if}
          </div>
          <div class="txt">
            <div class="t ellipsis">{song.title}</div>
            <div class="a ellipsis">
              {song.artist || "Desconocido"}{#if showFolder && song.kind === "local"}<span class="dim">{song.folder.split("/").pop()}</span>{/if}
            </div>
          </div>
          {#if onDownload}
            <button class="icon-btn dl" title="Bajar como MP3" onclick={(e) => { e.stopPropagation(); onDownload(song); }}>
              <Download size={16} />
            </button>
          {/if}
          <div class="date">{date(song)}</div>
          <div class="dur">{song.durationMs ? time(song.durationMs / 1000) : ""}</div>
        </div>
      {/each}
    </div>
  </div>

  {#if alphabet && letters.length > 4}
    <div class="az">
      {#each letters as [l, i]}
        <button onclick={() => jump(i, l)}>{l}</button>
      {/each}
    </div>
    {#if activeLetter}<div class="bubble">{activeLetter}</div>{/if}
  {/if}
</div>

<style>
  .wrap {
    position: relative;
    height: 100%;
    min-height: 0;
  }
  .scroller {
    height: 100%;
    overflow-y: auto;
    padding: 0 24px 24px 32px;
  }
  .rows {
    position: relative;
  }
  .row {
    position: absolute;
    left: 0;
    right: 0;
    height: 56px;
    display: grid;
    grid-template-columns: 28px 42px minmax(0, 1fr) auto 56px 56px;
    align-items: center;
    gap: 12px;
    padding: 0 12px 0 4px;
    border-radius: var(--radius);
    cursor: default;
    transition: background 0.12s;
  }
  .row:hover,
  .row:focus-visible {
    background: rgba(255, 255, 255, 0.06);
  }
  .row.current {
    background: rgba(255, 255, 255, 0.08);
  }
  .row.current .t {
    color: var(--accent);
  }
  .menu-btn {
    width: 28px;
    height: 28px;
    border-radius: 50%;
    display: grid;
    place-items: center;
    color: var(--text-3);
    opacity: 0;
    transition: opacity 0.12s, background 0.12s;
  }
  .row:hover .menu-btn {
    opacity: 1;
  }
  .menu-btn:hover {
    background: rgba(255, 255, 255, 0.1);
    color: var(--text);
  }
  .cov {
    position: relative;
  }
  .eq {
    position: absolute;
    z-index: 1;
    inset: 0;
    display: grid;
    place-items: center;
    background: rgba(0, 0, 0, 0.5);
    border-radius: 8px;
    color: var(--accent);
  }
  .eq :global(svg) {
    animation: pulse 1.1s ease-in-out infinite;
  }
  .eq.paused :global(svg) {
    animation: none;
  }
  @keyframes pulse {
    50% {
      transform: scaleY(0.6);
    }
  }
  .t {
    font-weight: 540;
    font-size: 14px;
  }
  .a {
    color: var(--text-2);
    font-size: 12.5px;
    margin-top: 2px;
  }
  .dim {
    color: var(--text-3);
  }
  .dim::before {
    content: "·";
    margin: 0 5px;
  }
  .dl {
    width: 30px;
    height: 30px;
  }
  .date,
  .dur {
    color: var(--text-3);
    font-size: 12.5px;
    font-variant-numeric: tabular-nums;
    text-align: right;
  }
  .az {
    position: absolute;
    right: 4px;
    top: 50%;
    transform: translateY(-50%);
    display: flex;
    flex-direction: column;
    padding: 6px 2px;
    border-radius: 12px;
    background: rgba(0, 0, 0, 0.18);
  }
  .az button {
    width: 20px;
    height: 17px;
    font-size: 10.5px;
    font-weight: 600;
    color: var(--text-3);
    border-radius: 4px;
  }
  .az button:hover {
    color: var(--accent);
  }
  .bubble {
    position: absolute;
    right: 44px;
    top: 50%;
    transform: translateY(-50%);
    width: 64px;
    height: 64px;
    border-radius: 16px;
    display: grid;
    place-items: center;
    font-size: 30px;
    font-weight: 700;
    color: var(--accent);
    background: var(--glass-strong);
    backdrop-filter: blur(20px);
    pointer-events: none;
  }
</style>
