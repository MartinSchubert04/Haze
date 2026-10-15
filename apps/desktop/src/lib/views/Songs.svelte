<script lang="ts">
  import { FolderPlus, TvMinimalPlay } from "@lucide/svelte";
  import { store } from "../store.svelte";
  import { normalizeSearch, plural } from "../format";
  import SongList from "../components/SongList.svelte";
  import ListHeader from "../components/ListHeader.svelte";

  const q = $derived(normalizeSearch(store.search.trim()));
  const filtered = $derived.by(() => {
    const list = q
      ? store.songs.filter((s) => normalizeSearch(`${s.title} ${s.artist} ${s.album}`).includes(q))
      : store.songs;
    return store.sortSongs(list);
  });
  const sort = $derived(store.settings?.songSort ?? "title_asc");
  const sub = $derived(
    (q ? `${plural(filtered.length, "resultado", "resultados")} para "${store.search.trim()}"` : plural(store.songs.length, "canción", "canciones")) +
      (store.scanning ? " · escaneando…" : ""),
  );
</script>

{#if store.songs.length === 0 && !store.scanning}
  <div class="empty">
    <h3>Todavía no hay música</h3>
    <p>Agregá las carpetas donde tenés tus canciones, o buscá algo en YouTube.</p>
    <div style="display:flex;gap:10px">
      <button class="pill primary" onclick={() => store.go({ name: "settings" })}><FolderPlus size={16} /> Agregar carpetas</button>
      <button class="pill" onclick={() => store.go({ name: "youtube" })}><TvMinimalPlay size={16} /> Ir a YouTube</button>
    </div>
  </div>
{:else}
  <SongList songs={filtered} context={q ? `Búsqueda: ${store.search.trim()}` : "Todas las canciones"} alphabet={sort.startsWith("title")}>
    {#snippet header()}
      <ListHeader title={q ? "Resultados" : "Canciones"} {sub} songs={filtered} context="Todas las canciones">
        {#snippet extra()}
          {#if q}
            <button class="pill" onclick={() => store.go({ name: "youtube", query: store.search.trim() })}>
              <TvMinimalPlay size={16} color="#ff5f5f" /> Buscar en YouTube
            </button>
          {/if}
        {/snippet}
      </ListHeader>
    {/snippet}
  </SongList>
{/if}
