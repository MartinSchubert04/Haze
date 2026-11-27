<script lang="ts">
  // Opciones de una cancion: se abre al mantenerla apretada.
  import { ListPlus, ListEnd, FolderOpen, Trash2, Download, Library, ListX, EyeOff } from "@lucide/svelte";
  import { folderName } from "../format";
  import { api, type Song } from "../api";
  import { store } from "../store.svelte";
  import Cover from "./Cover.svelte";

  let { song, queueIndex, listId }: { song: Song; queueIndex?: number; listId?: string } = $props();

  // Listas y bajas nombran el archivo por su clave, que tienen los temas del telefono (no
  // los de YouTube sin bajar).
  const keeps = $derived(song.kind !== "stream" && !!song.file);
  const list = $derived(listId ? store.list(listId) : undefined);

  // Dar de baja pide un segundo toque y explica que no se borra nada.
  let confirming = $state(false);

  // Las props salen de la capa abierta: despues de store.close() ya no existe y leerlas
  // tira una excepcion. Por eso cada accion toma lo que necesita antes de cerrar (asi
  // "Bajar como MP3" no hacia nada).
  async function enqueue(next: boolean) {
    const s = song;
    store.close();
    await store.enqueue(s, next);
  }

  // Un resultado de busqueda ya trae titulo y tapa: se baja directo, de fondo. El avance
  // se ve en la notificacion y en la pestaña YouTube.
  function download() {
    const { path, title, thumb } = song;
    store.close();
    store.download(path, title, thumb);
  }

  function remove() {
    const index = queueIndex;
    store.close();
    if (index !== undefined) api.queueAction("remove", index);
  }

  function addToList() {
    const s = song;
    store.replace({ kind: "addToList", songs: [s] });
  }

  function removeFromList() {
    const l = list;
    const file = song.file;
    store.close();
    if (l) store.editUser({ type: "remove", id: l.id, file });
  }

  function giveUp() {
    const s = song;
    store.close();
    store.removeSongs([s]);
  }
</script>

<div class="backdrop" role="presentation" onclick={() => store.close()}></div>
<div class="sheet" role="dialog" aria-label="{song.title} options">
  <div class="grip"></div>
  <div class="head">
    <Cover {song} size={52} radius={8} />
    <div class="text">
      <div class="title ellipsis">{song.title}</div>
      <div class="sub ellipsis">{song.artist || "Unknown artist"}</div>
    </div>
  </div>
  {#if confirming}
    <p class="hint">
      It's hidden on the PC, the phone and the TV, and won't be downloaded again. The file isn't deleted: you can restore it in Settings.
    </p>
    <button class="opt press danger" onclick={giveUp}><EyeOff size={20} /> Yes, remove it</button>
    <button class="opt press" onclick={() => (confirming = false)}>Cancel</button>
  {:else}
    <button class="opt press" onclick={() => enqueue(true)}><ListPlus size={20} /> Play next</button>
    <button class="opt press" onclick={() => enqueue(false)}><ListEnd size={20} /> Add to queue</button>
    {#if song.kind === "stream"}
      <button class="opt press" onclick={download}><Download size={20} /> Download as MP3</button>
    {:else if store.openFolder !== song.folder}
      <button class="opt press" onclick={() => store.goToFolder(song.folder)}>
        <FolderOpen size={20} /> Go to {folderName(song.folder) || "folder"}
      </button>
    {/if}
    {#if keeps}
      <button class="opt press" onclick={addToList}><Library size={20} /> Add to list…</button>
    {/if}
    {#if list && keeps}
      <button class="opt press danger" onclick={removeFromList}><ListX size={20} /> Remove from list</button>
    {/if}
    {#if queueIndex !== undefined}
      <button class="opt press danger" onclick={remove}><Trash2 size={20} /> Remove from queue</button>
    {/if}
    {#if keeps}
      <button class="opt press danger" onclick={() => (confirming = true)}><EyeOff size={20} /> Remove everywhere</button>
    {/if}
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
