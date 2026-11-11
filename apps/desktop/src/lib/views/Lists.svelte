<script lang="ts">
  // Las listas de Haze: viven en el server (por usuario) y se editan aca aunque no haya
  // conexion. Las canciones no se copian: la lista solo dice cuales son.
  import { Plus, Pencil, Trash2, Library } from "@lucide/svelte";
  import { store } from "../store.svelte";
  import { menu } from "../menu.svelte";
  import { normalizeSearch, plural } from "@haze/ui/format";
  import type { List, Song } from "../api";
  import Cover from "../components/Cover.svelte";

  const q = $derived(normalizeSearch(store.search.trim()));
  const lists = $derived(q ? store.lists.filter((l) => normalizeSearch(l.name).includes(q)) : store.lists);

  // Hasta 4 tapas distintas para el mosaico.
  function covers(songs: Song[]) {
    const seen = new Set<string>();
    const out = [];
    for (const s of songs) {
      if (!s.hasCover) continue;
      const key = s.album || s.path;
      if (seen.has(key)) continue;
      seen.add(key);
      out.push(s);
      if (out.length === 4) break;
    }
    return out;
  }

  function listMenu(e: MouseEvent, l: List) {
    menu.show(e, [
      { label: "Renombrar", icon: Pencil, action: () => store.renameList(l) },
      { label: "Borrar lista", icon: Trash2, danger: true, action: () => store.deleteList(l) },
    ]);
  }
</script>

<div class="scroll">
  <div class="page">
    <div class="top">
      <div>
        <h1 class="page-title">Listas</h1>
        <div class="page-sub">{plural(lists.length, "lista", "listas")}</div>
      </div>
      <button class="pill" onclick={() => store.newList()}><Plus size={16} /> Nueva lista</button>
    </div>

    {#if store.lists.length === 0}
      <div class="empty">
        <Library size={28} />
        <h3>Todavía no hay listas</h3>
        <p>Creá una, o agregá canciones desde su menú (clic derecho → Agregar a lista).</p>
      </div>
    {:else}
      <div class="grid">
        {#each lists as l (l.id)}
          {@const songs = store.listSongs(l)}
          {@const cs = covers(songs)}
          <button class="card" onclick={() => store.go({ name: "list", id: l.id })} oncontextmenu={(e) => listMenu(e, l)}>
            <div class="art" class:mosaic={cs.length >= 4}>
              {#if cs.length >= 4}
                {#each cs as s}<Cover song={s} size="100%" radius={0} />{/each}
              {:else}
                <Cover song={cs[0] ?? null} size="100%" radius={0} />
              {/if}
            </div>
            <div class="name ellipsis">{l.name}</div>
            <div class="count">
              {plural(songs.length, "canción", "canciones")}{#if l.songs.length > songs.length} · {l.songs.length - songs.length} sin bajar{/if}
            </div>
          </button>
        {/each}
      </div>
    {/if}
  </div>
</div>

<style>
  .scroll {
    height: 100%;
    overflow-y: auto;
  }
  .top {
    display: flex;
    align-items: flex-end;
    justify-content: space-between;
  }
  .top .pill {
    margin-bottom: 16px;
  }
  .empty :global(svg) {
    color: var(--text-3);
  }
  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(168px, 1fr));
    gap: 14px;
  }
  .card {
    text-align: left;
    padding: 10px 10px 14px;
    border-radius: var(--radius-lg);
    background: var(--tile);
    transition: background 0.15s, transform 0.15s;
    min-width: 0;
  }
  .card:hover {
    background: rgb(var(--ov) / 0.07);
  }
  .card:active {
    transform: scale(0.985);
  }
  .art {
    position: relative;
    aspect-ratio: 1;
    border-radius: calc(12px * var(--rs));
    overflow: hidden;
    margin-bottom: 12px;
    background: color-mix(in srgb, var(--folder) 14%, transparent);
    color: var(--folder);
  }
  .art :global(.cover) {
    background: transparent;
    color: color-mix(in srgb, var(--folder) 80%, transparent);
  }
  .mosaic {
    display: grid;
    grid-template-columns: 1fr 1fr;
  }
  .name {
    font-weight: 600;
    font-size: 14px;
  }
  .count {
    color: var(--text-2);
    font-size: 12.5px;
    margin-top: 3px;
  }
</style>
