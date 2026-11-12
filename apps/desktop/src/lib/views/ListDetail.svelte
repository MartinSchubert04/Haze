<script lang="ts">
  import { Pencil, Trash2, ListX } from "@lucide/svelte";
  import { store } from "../store.svelte";
  import { normalizeSearch, plural } from "@haze/ui/format";
  import { fileKey } from "@haze/ui/library";
  import type { Song } from "../api";
  import SongList from "../components/SongList.svelte";
  import ListHeader, { SONG_SORTS } from "../components/ListHeader.svelte";

  let { id }: { id: string } = $props();

  const SORTS: [string, string][] = [["custom", "Orden de la lista"], ...SONG_SORTS];

  const list = $derived(store.list(id));
  // "custom": en el orden de la lista, que se cambia arrastrando. Si no, uno propio de las
  // listas (aparte del de Canciones).
  const sort = $derived(store.settings?.listSongSort ?? "custom");
  const custom = $derived(sort === "custom");
  const songs = $derived.by(() => {
    const l = list ? store.listSongs(list) : [];
    return custom ? l : store.sortSongs(l, sort);
  });
  const q = $derived(normalizeSearch(store.search.trim()));
  const shown = $derived(q ? songs.filter((s) => normalizeSearch(`${s.title} ${s.artist}`).includes(q)) : songs);
  const missing = $derived(list ? list.songs.length - songs.length : 0);
  // Arrastrar solo con el orden de la lista y sin busqueda: si no, lo que se ve no es ese orden.
  const movable = $derived(custom && !q && songs.length > 1);
  const sub = $derived(
    plural(songs.length, "canción", "canciones") +
      (missing > 0 ? ` · ${missing} todavía no llegaron a esta PC` : "") +
      (movable ? " · arrastrá para cambiar el orden" : ""),
  );

  function move(from: number, to: number) {
    if (list) store.editUser({ type: "move", id: list.id, file: fileKey(shown[from]), to: fileKey(shown[to]) });
  }

  function removeFrom(s: Song) {
    if (list) store.editUser({ type: "remove", id: list.id, file: fileKey(s) });
  }
</script>

{#if list}
  <SongList
    songs={shown}
    context={list.name}
    alphabet={sort.startsWith("title")}
    menuExtra={(s) => [{ label: "Quitar de la lista", icon: ListX, action: () => removeFrom(s) }]}
    onMove={movable ? move : undefined}
  >
    {#snippet header()}
      <ListHeader title={list.name} {sub} songs={shown} context={list.name}
        sorts={SORTS}
        {sort}
        onsort={(k) => store.saveSettings((s) => (s.listSongSort = k))}
      >
        {#snippet extra()}
          <button class="pill" onclick={() => store.renameList(list)}><Pencil size={15} /> Renombrar</button>
          <button class="pill" onclick={() => store.deleteList(list)}><Trash2 size={15} /> Borrar</button>
        {/snippet}
      </ListHeader>
      {#if songs.length === 0}
        <p class="hint">Vacía. Agregá canciones desde su menú (clic derecho → Agregar a lista).</p>
      {/if}
    {/snippet}
  </SongList>
{:else}
  <div class="empty">
    <h3>Esta lista ya no existe</h3>
    <button class="pill" onclick={() => store.go({ name: "lists" })}>Ver las listas</button>
  </div>
{/if}

<style>
  .hint {
    color: var(--text-2);
    font-size: 13px;
    margin: 4px 0 0;
  }
</style>
