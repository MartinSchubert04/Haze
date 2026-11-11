<script lang="ts">
  import { ChevronLeft, EllipsisVertical, Play, Shuffle } from "@lucide/svelte";
  import { plural } from "@haze/ui/format";
  import { api } from "../api";
  import { store } from "../store.svelte";
  import SongList from "../components/SongList.svelte";

  let { id }: { id: string } = $props();

  const list = $derived(store.list(id));
  const name = $derived(list?.name ?? "Lista");
  // En el orden de la lista, no en el de la biblioteca.
  const songs = $derived(list ? store.listSongs(list) : []);

  async function play(shuffle: boolean) {
    if (store.player.shuffle !== shuffle) await api.control({ type: "shuffle", value: shuffle });
    store.play(songs, shuffle ? Math.floor(Math.random() * songs.length) : 0, name);
  }
</script>

<section class="layer">
  {#snippet header()}
    <div class="page">
      <div class="top">
        <button class="icon-btn back" aria-label="Volver" onclick={() => store.close()}><ChevronLeft size={26} /></button>
        {#if list}
          <button class="icon-btn more" aria-label="Opciones" onclick={() => store.open({ kind: "listOptions", id })}>
            <EllipsisVertical size={22} />
          </button>
        {/if}
      </div>
      <h1 class="page-title">{name}</h1>
      <div class="page-sub">{plural(songs.length, "canción", "canciones")} · mantené apretada una para quitarla</div>
      <div class="actions">
        <button class="pill primary" disabled={!songs.length} onclick={() => play(false)}><Play size={16} fill="currentColor" /> Reproducir</button>
        <button class="pill" disabled={!songs.length} onclick={() => play(true)}><Shuffle size={16} /> Aleatorio</button>
      </div>
    </div>
    {#if !songs.length}
      <div class="empty">
        <h3>Lista vacía</h3>
        <p>Mantené apretada una canción y elegí "Agregar a lista…".</p>
      </div>
    {/if}
  {/snippet}
  <SongList {songs} context={name} {header} listId={id} onrefresh={() => api.rescan()} />
</section>

<style>
  .layer {
    position: fixed;
    inset: 0;
    z-index: 10;
    background: var(--bg);
    animation: in 0.2s ease-out;
  }
  @keyframes in {
    from {
      transform: translateX(24px);
      opacity: 0;
    }
  }
  .top {
    display: flex;
    justify-content: space-between;
    min-height: 44px;
  }
  .back {
    margin-left: -10px;
  }
  .more {
    margin-right: -10px;
  }
  .page-sub {
    margin-bottom: 14px;
  }
  .actions {
    display: flex;
    gap: 10px;
  }
</style>
