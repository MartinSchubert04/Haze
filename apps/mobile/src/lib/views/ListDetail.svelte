<script lang="ts">
  import { ChevronLeft, EllipsisVertical, Play, Shuffle, ListOrdered, Clock, ArrowDownAZ, UserRound } from "@lucide/svelte";
  import { plural } from "@haze/ui/format";
  import { api } from "../api";
  import { store } from "../store.svelte";
  import SongList from "../components/SongList.svelte";
  import Select from "../components/Select.svelte";

  const SORTS = [
    { id: "custom", label: "De la lista", icon: ListOrdered },
    { id: "date_desc", label: "Recientes", icon: Clock },
    { id: "title_asc", label: "Título", icon: ArrowDownAZ },
    { id: "artist_asc", label: "Artista", icon: UserRound },
  ];

  let { id }: { id: string } = $props();

  const list = $derived(store.list(id));
  const name = $derived(list?.name ?? "Lista");
  // "custom": en el orden de la lista. Si no, uno propio de las listas (aparte del de Canciones).
  const sort = $derived(store.settings?.listSongSort ?? "custom");
  const songs = $derived.by(() => {
    const l = list ? store.listSongs(list) : [];
    return sort === "custom" ? l : store.sortSongs(l, sort);
  });

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
      {#if songs.length > 1}
        <div class="sort">
          <Select label="Ordenar" value={sort} options={SORTS} onchange={(k) => store.saveSettings((x) => (x.listSongSort = k))} />
        </div>
      {/if}
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
  .sort {
    margin-top: 18px;
  }
</style>
