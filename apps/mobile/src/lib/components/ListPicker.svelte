<script lang="ts">
  // "Agregar a lista…": elegir la lista, o crear una nueva con estas canciones.
  import { Check, ListMusic, Plus } from "@lucide/svelte";
  import { fileKey } from "@haze/ui/library";
  import { plural } from "@haze/ui/format";
  import type { List, Song } from "../api";
  import { store } from "../store.svelte";

  let { songs }: { songs: Song[] } = $props();

  const keys = $derived(songs.map(fileKey));
  // Ya estan todas: tocarla no cambiaria nada.
  const has = (l: List) => keys.every((k) => l.songs.includes(k));

  // `songs` sale de la capa: se lee antes de cerrarla (despues ya no existe).
  function add(l: List) {
    const s = songs;
    store.close();
    store.addToList(l, s);
  }

  function create() {
    const s = songs;
    store.replace({ kind: "listName", songs: s });
  }
</script>

<div class="backdrop" role="presentation" onclick={() => store.close()}></div>
<div class="sheet" role="dialog" aria-label="Agregar a lista">
  <div class="grip"></div>
  <div class="head">
    <div class="title">Agregar a lista</div>
    <div class="sub ellipsis">{songs.length === 1 ? songs[0].title : plural(songs.length, "canción", "canciones")}</div>
  </div>
  <div class="lists">
    <button class="opt press" onclick={create}>
      <Plus size={20} /> {songs.length === 1 ? "Nueva lista con este tema" : "Nueva lista con estos temas"}
    </button>
    {#each store.lists as l (l.id)}
      <button class="opt press" disabled={has(l)} onclick={() => add(l)}>
        <ListMusic size={20} />
        <span class="ellipsis grow">{l.name}</span>
        {#if has(l)}<Check size={18} />{/if}
      </button>
    {/each}
  </div>
</div>

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    z-index: 60;
    background: rgba(0, 0, 0, 0.5);
    animation: fade 0.2s ease-out;
  }
  .sheet {
    position: fixed;
    /* --pop puede ser translucido: debajo va el fondo del tema, asi no se ven las
       pestañas a traves. */
    background-color: var(--bg);
    left: 0;
    right: 0;
    bottom: 0;
    z-index: 61;
    display: flex;
    flex-direction: column;
    max-height: 80%;
    padding: 8px 8px calc(var(--safe-bottom) + 12px);
    border-radius: calc(20px * var(--rs)) calc(20px * var(--rs)) 0 0;
    background-image: linear-gradient(var(--pop), var(--pop));
    border-top: 1px solid var(--line-strong);
    animation: up 0.22s cubic-bezier(0.2, 0.8, 0.2, 1);
  }
  .grip {
    flex: none;
    width: 36px;
    height: 4px;
    border-radius: calc(4px * var(--rs));
    background: var(--line-strong);
    margin: 0 auto 10px;
  }
  .head {
    flex: none;
    padding: 6px 12px 14px;
    border-bottom: 1px solid var(--line);
    margin-bottom: 6px;
    min-width: 0;
  }
  .title {
    font-weight: 600;
  }
  .sub {
    font-size: 13px;
    color: var(--text-2);
  }
  .lists {
    overflow-y: auto;
  }
  .opt {
    width: 100%;
    display: flex;
    align-items: center;
    gap: 16px;
    padding: 14px 12px;
    border-radius: calc(12px * var(--rs));
    text-align: left;
  }
  .opt:disabled {
    color: var(--text-3);
  }
  .grow {
    flex: 1;
    min-width: 0;
  }
  @keyframes up {
    from {
      transform: translateY(100%);
    }
  }
  @keyframes fade {
    from {
      opacity: 0;
    }
  }
</style>
