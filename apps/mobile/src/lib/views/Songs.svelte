<script lang="ts">
  import { Search, X, Shuffle, LoaderCircle } from "@lucide/svelte";
  import { normalizeSearch, songCount } from "../format";
  import { api } from "../api";
  import { store, ALL_SONGS } from "../store.svelte";
  import SongList from "../components/SongList.svelte";
  import Select from "../components/Select.svelte";
  import GearMenu from "../components/GearMenu.svelte";
  import { SONG_SORTS } from "../sorts";

  let query = $state("");

  // Con una busqueda escrita, "atras" la borra y vuelve al listado entero.
  $effect(() => {
    if (!query) return;
    return store.onBack("page", () => {
      query = "";
      (document.activeElement as HTMLElement | null)?.blur();
      return true;
    });
  });

  const sorted = $derived(store.sortSongs(store.uniqueSongs));
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
      <h1 class="page-title">Songs</h1>
      <GearMenu />
    </div>
    <div class="page-sub">
      {#if store.scanning}<LoaderCircle size={13} class="spin" /> Looking for music…
      {:else}{songCount(store.uniqueSongs.length)}{/if}
    </div>

    <label class="search">
      <Search size={18} />
      <input type="search" placeholder="Search" bind:value={query} />
      {#if query}<button class="tap" aria-label="Clear" onclick={() => (query = "")}><X size={18} /></button>{/if}
    </label>

    <div class="actions">
      <button class="pill primary" disabled={!sorted.length} onclick={shuffleAll}><Shuffle size={16} /> Shuffle</button>
      <Select
        label="Sort"
        value={store.settings?.songSort ?? "date_desc"}
        options={SONG_SORTS}
        onchange={(id) => store.saveSettings((x) => (x.songSort = id))}
      />
    </div>
  </div>
  {#if !store.scanning && !store.songs.length}
    <div class="empty">
      <h3>No music yet</h3>
      <p>Copy some songs to your phone (to the Music folder, for example) and open Haze again.</p>
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
    border-radius: calc(14px * var(--rs));
    background: rgb(var(--ov) / 0.07);
    border: var(--border) solid var(--line);
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
