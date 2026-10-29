<script lang="ts">
  // Opciones de una cancion: se abre al mantenerla apretada.
  import { ListPlus, ListEnd, FolderOpen, Trash2, Download } from "@lucide/svelte";
  import { folderName } from "@haze/ui/format";
  import { api, type Song } from "../api";
  import { store } from "../store.svelte";
  import Cover from "./Cover.svelte";

  let { song, queueIndex }: { song: Song; queueIndex?: number } = $props();

  async function enqueue(next: boolean) {
    store.close();
    await store.enqueue(song, next);
  }

  function download() {
    store.close();
    store.download(song.path, song.title, song.thumb);
  }

  function remove() {
    store.close();
    if (queueIndex !== undefined) api.queueAction("remove", queueIndex);
  }
</script>

<div class="backdrop" role="presentation" onclick={() => store.close()}></div>
<div class="sheet" role="dialog" aria-label="Opciones de {song.title}">
  <div class="grip"></div>
  <div class="head">
    <Cover {song} size={52} radius={8} />
    <div class="text">
      <div class="title ellipsis">{song.title}</div>
      <div class="sub ellipsis">{song.artist || "Artista desconocido"}</div>
    </div>
  </div>
  <button class="opt press" onclick={() => enqueue(true)}><ListPlus size={20} /> Reproducir a continuación</button>
  <button class="opt press" onclick={() => enqueue(false)}><ListEnd size={20} /> Agregar a la cola</button>
  {#if song.kind === "stream"}
    <button class="opt press" onclick={download}><Download size={20} /> Bajar como MP3</button>
  {:else if store.openFolder !== song.folder}
    <button class="opt press" onclick={() => store.goToFolder(song.folder)}>
      <FolderOpen size={20} /> Ir a {folderName(song.folder) || "la carpeta"}
    </button>
  {/if}
  {#if queueIndex !== undefined}
    <button class="opt press danger" onclick={remove}><Trash2 size={20} /> Quitar de la cola</button>
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
    left: 0;
    right: 0;
    bottom: 0;
    z-index: 61;
    padding: 8px 8px calc(var(--safe-bottom) + 12px);
    border-radius: calc(20px * var(--rs)) calc(20px * var(--rs)) 0 0;
    background: var(--pop);
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
