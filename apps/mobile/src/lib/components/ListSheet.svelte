<script lang="ts">
  // Opciones de una lista: se abre al mantenerla apretada o desde su pantalla.
  import { ListMusic, FolderOpen, Pencil, Trash2 } from "@lucide/svelte";
  import { plural } from "@haze/ui/format";
  import { store } from "../store.svelte";
  import Cover from "./Cover.svelte";

  let { id }: { id: string } = $props();

  const list = $derived(store.list(id));
  const songs = $derived(list ? store.listSongs(list) : []);
  const coverSong = $derived(songs.find((s) => s.hasCover) ?? null);

  // Borrar pide un segundo toque: la lista se va de todos los aparatos.
  let confirming = $state(false);

  // `id` sale de la capa: se lee antes de reemplazarla (despues ya no existe).
  function open() {
    const listId = id;
    store.replace({ kind: "list", id: listId });
  }

  function rename() {
    const listId = id;
    store.replace({ kind: "listName", id: listId });
  }

  function remove() {
    const l = list;
    if (l) store.deleteList(l);
    else store.close();
  }
</script>

<div class="backdrop" role="presentation" onclick={() => store.close()}></div>
<div class="sheet" role="dialog" aria-label="Opciones de {list?.name ?? 'la lista'}">
  <div class="grip"></div>
  <div class="head">
    {#if coverSong}
      <Cover song={coverSong} size={52} radius={8} />
    {:else}
      <div class="icon"><ListMusic size={24} /></div>
    {/if}
    <div class="text">
      <div class="title ellipsis">{list?.name ?? "Lista"}</div>
      <div class="sub ellipsis">{plural(songs.length, "canción", "canciones")}</div>
    </div>
  </div>
  {#if confirming}
    <p class="hint">Se borra la lista en todos los aparatos. Las canciones quedan.</p>
    <button class="opt press danger" onclick={remove}><Trash2 size={20} /> Sí, borrar {list?.name ?? "la lista"}</button>
    <button class="opt press" onclick={() => (confirming = false)}>Cancelar</button>
  {:else}
    {#if store.openList !== id}
      <button class="opt press" onclick={open}><FolderOpen size={20} /> Abrir</button>
    {/if}
    <button class="opt press" onclick={rename}><Pencil size={20} /> Renombrar</button>
    <button class="opt press danger" onclick={() => (confirming = true)}><Trash2 size={20} /> Borrar lista</button>
  {/if}
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
    padding: 8px 8px calc(var(--safe-bottom) + 12px);
    border-radius: calc(20px * var(--rs)) calc(20px * var(--rs)) 0 0;
    background-image: linear-gradient(var(--pop), var(--pop));
    border-top: 1px solid var(--line-strong);
    animation: up 0.22s cubic-bezier(0.2, 0.8, 0.2, 1);
  }
  .grip {
    width: 36px;
    height: 4px;
    border-radius: calc(4px * var(--rs));
    background: var(--line-strong);
    margin: 0 auto 10px;
  }
  .head {
    display: flex;
    align-items: center;
    gap: 14px;
    padding: 6px 12px 14px;
    border-bottom: 1px solid var(--line);
    margin-bottom: 6px;
  }
  .icon {
    width: 52px;
    height: 52px;
    flex: none;
    display: grid;
    place-items: center;
    border-radius: calc(8px * var(--rs));
    background: rgb(var(--ov) / 0.06);
    color: var(--accent);
  }
  .text {
    min-width: 0;
  }
  .title {
    font-weight: 600;
  }
  .sub {
    font-size: 13px;
    color: var(--text-2);
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
  .danger {
    color: var(--danger);
  }
  .hint {
    margin: 6px 12px 4px;
    font-size: 13px;
    color: var(--text-2);
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
