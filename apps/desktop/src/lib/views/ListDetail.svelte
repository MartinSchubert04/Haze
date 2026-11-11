<script lang="ts">
  import { Pencil, Trash2, ListX } from "@lucide/svelte";
  import { store } from "../store.svelte";
  import { normalizeSearch, plural } from "@haze/ui/format";
  import { fileKey } from "@haze/ui/library";
  import type { Song } from "../api";
  import SongList from "../components/SongList.svelte";
  import ListHeader from "../components/ListHeader.svelte";

  let { id }: { id: string } = $props();

  const list = $derived(store.list(id));
  // En el orden de la lista: no se reordena con el orden de Canciones.
  const songs = $derived(list ? store.listSongs(list) : []);
  const q = $derived(normalizeSearch(store.search.trim()));
  const shown = $derived(q ? songs.filter((s) => normalizeSearch(`${s.title} ${s.artist}`).includes(q)) : songs);
  const missing = $derived(list ? list.songs.length - songs.length : 0);
  const sub = $derived(plural(songs.length, "canción", "canciones") + (missing > 0 ? ` · ${missing} todavía no llegaron a esta PC` : ""));

  function removeFrom(s: Song) {
    if (list) store.editUser({ type: "remove", id: list.id, file: fileKey(s) });
  }
</script>

{#if list}
  <SongList
    songs={shown}
    context={list.name}
    menuExtra={(s) => [{ label: "Quitar de la lista", icon: ListX, action: () => removeFrom(s) }]}
  >
    {#snippet header()}
      <ListHeader title={list.name} {sub} songs={shown} context={list.name} sortable={false}>
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
