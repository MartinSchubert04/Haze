<script lang="ts">
  // Opciones de una carpeta: se abre al mantenerla apretada o desde su pantalla.
  import { Folder, FolderOpen, EyeOff } from "@lucide/svelte";
  import { folderName, plural } from "@haze/ui/format";
  import { store } from "../store.svelte";
  import Cover from "./Cover.svelte";

  let { path }: { path: string } = $props();

  const folder = $derived(store.folders.find((f) => f.path === path));
  const name = $derived(folderName(path) || "Almacenamiento");

  // `path` sale de la capa: se lee antes de cerrarla (despues ya no existe).
  function open() {
    const p = path;
    store.replace({ kind: "folder", path: p });
  }
</script>

<div class="backdrop" role="presentation" onclick={() => store.close()}></div>
<div class="sheet" role="dialog" aria-label="Opciones de {name}">
  <div class="grip"></div>
  <div class="head">
    {#if folder?.coverSong}
      <Cover song={folder.coverSong} size={52} radius={8} />
    {:else}
      <div class="icon"><Folder size={24} /></div>
    {/if}
    <div class="text">
      <div class="title ellipsis">{name}</div>
      <div class="sub ellipsis">{plural(folder?.songs.length ?? 0, "canción", "canciones")} · {path || "/"}</div>
    </div>
  </div>
  {#if store.openFolder !== path}
    <button class="opt press" onclick={open}><FolderOpen size={20} /> Abrir</button>
  {/if}
  <button class="opt press danger" onclick={() => store.hideFolder(path)}>
    <EyeOff size={20} /> Quitar de la biblioteca
  </button>
  <p class="hint">No se borra nada del teléfono. Se vuelve a mostrar desde Ajustes.</p>
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
    color: var(--folder);
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
    margin: 2px 12px 0;
    font-size: 12.5px;
    color: var(--text-3);
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
