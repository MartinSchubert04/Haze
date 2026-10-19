<script lang="ts">
  import { Search, X, Settings as SettingsIcon, Shuffle, LoaderCircle, Clock, ArrowDownAZ, UserRound } from "@lucide/svelte";
  import { normalizeSearch, plural } from "@haze/ui/format";
  import { api } from "../api";
  import { store, ALL_SONGS } from "../store.svelte";
  import SongList from "../components/SongList.svelte";
  import Select from "../components/Select.svelte";

  const SORTS = [
    { id: "date_desc", label: "Recientes", icon: Clock },
    { id: "title_asc", label: "Título", icon: ArrowDownAZ },
    { id: "artist_asc", label: "Artista", icon: UserRound },
  ];

  let query = $state("");

  const sorted = $derived(store.sortSongs(store.songs));
  const shown = $derived.by(() => {
    const q = normalizeSearch(query.trim());
    if (!q) return sorted;
    return sorted.filter((s) => normalizeSearch(`${s.title} ${s.artist} ${s.album}`).includes(q));
  });

  async function shuffleAll() {
    if (!store.player.shuffle) await api.control({ type: "shuffle", value: true });
    const start = Math.floor(Math.random() * sorted.length);
    store.play(sorted, start, ALL_SONGS);
  }
</script>

{#snippet header()}
  <div class="page">
    <div class="top">
      <h1 class="page-title">Canciones</h1>
      <button class="icon-btn" aria-label="Ajustes" onclick={() => store.open({ kind: "settings" })}>
        <SettingsIcon size={22} />
      </button>
    </div>
    <div class="page-sub">
      {#if store.scanning}<LoaderCircle size={13} class="spin" /> Buscando música…
      {:else}{plural(store.songs.length, "canción", "canciones")}{/if}
    </div>

    <label class="search">
      <Search size={18} />
      <input type="search" placeholder="Buscar" bind:value={query} />
      {#if query}<button aria-label="Borrar" onclick={() => (query = "")}><X size={18} /></button>{/if}
    </label>

    <div class="actions">
      <button class="pill primary" disabled={!sorted.length} onclick={shuffleAll}><Shuffle size={16} /> Aleatorio</button>
      <Select
        label="Ordenar"
        value={store.settings?.songSort ?? "date_desc"}
        options={SORTS}
        onchange={(id) => store.saveSettings((x) => (x.songSort = id))}
      />
    </div>
  </div>
  {#if !store.scanning && !store.songs.length}
    <div class="empty">
      <h3>No hay música</h3>
      <p>Copiá canciones al teléfono (por ejemplo, a la carpeta Music) y volvé a abrir Haze.</p>
    </div>
  {/if}
{/snippet}

<!-- Con una busqueda, tocar un tema lo hace sonar sin cambiar la cola por los resultados. -->
<SongList
  songs={shown}
  context={ALL_SONGS}
  {header}
  onrefresh={() => api.rescan()}
  onplay={query.trim() ? (song) => store.playNow(song) : undefined}
/>

<style>
  .top {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }
  .page-sub {
    display: flex;
    align-items: center;
    gap: 6px;
    margin-bottom: 14px;
  }
  .search {
    display: flex;
    align-items: center;
    gap: 10px;
    height: 46px;
    padding: 0 14px;
    border-radius: 14px;
    background: rgba(255, 255, 255, 0.07);
    border: 1px solid var(--line);
    color: var(--text-2);
  }
  .search input {
    flex: 1;
    min-width: 0;
    background: none;
    border: 0;
    padding: 0;
    color: var(--text);
  }
  .search input::-webkit-search-cancel-button {
    display: none;
  }
  .actions {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
    margin-top: 18px;
  }
  .empty {
    padding: 40px 24px;
  }
</style>
