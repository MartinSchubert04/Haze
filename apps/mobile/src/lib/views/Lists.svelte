<script lang="ts">
  import { ListMusic, Plus, ArrowDownAZ, Hash, History } from "@lucide/svelte";
  import { plural } from "@haze/ui/format";
  import { sortLists } from "@haze/ui/library";
  import type { List } from "../api";
  import { store } from "../store.svelte";
  import { longpress } from "../longpress";
  import Cover from "../components/Cover.svelte";
  import Select from "../components/Select.svelte";

  const SORTS = [
    { id: "name_asc", label: "Nombre", icon: ArrowDownAZ },
    { id: "count_desc", label: "Más canciones", icon: Hash },
    { id: "updated_desc", label: "Cambiadas", icon: History },
  ];
  const sort = $derived(store.settings?.listSort ?? "name_asc");

  // Las canciones de cada lista que estan en el telefono; las que faltan llegan al
  // sincronizar en casa.
  const rows = $derived(
    sortLists(store.lists, sort, (l) => store.listSongs(l).length).map((list: List) => {
      const songs = store.listSongs(list);
      const missing = list.songs.filter((k) => !store.removed.has(k)).length - songs.length;
      return { list, songs, missing, coverSong: songs.find((s) => s.hasCover) ?? null };
    }),
  );
</script>

<div class="scroller">
  <div class="page">
    <h1 class="page-title">Listas</h1>
    <div class="page-sub">
      {plural(store.lists.length, "lista", "listas")}{#if store.lists.length} · mantené apretada una para renombrarla o borrarla{/if}
    </div>
    <div class="actions">
      <button class="pill primary" onclick={() => store.open({ kind: "listName" })}><Plus size={16} /> Nueva lista</button>
      {#if store.lists.length > 1}
        <Select label="Ordenar" value={sort} options={SORTS} onchange={(k) => store.saveSettings((x) => (x.listSort = k))} />
      {/if}
    </div>

    {#if !store.lists.length}
      <div class="empty">
        <h3>Todavía no hay listas</h3>
        <p>Creá una acá o mantené apretada una canción y elegí "Agregar a lista…". Las listas se comparten con la PC al sincronizar en casa.</p>
      </div>
    {/if}

    <div class="list">
      {#each rows as r (r.list.id)}
        <button
          class="row press"
          onclick={() => store.open({ kind: "list", id: r.list.id })}
          use:longpress={() => store.open({ kind: "listOptions", id: r.list.id })}
        >
          {#if r.coverSong}
            <Cover song={r.coverSong} size={56} radius={12} />
          {:else}
            <div class="icon"><ListMusic size={26} /></div>
          {/if}
          <div class="text">
            <div class="name ellipsis">{r.list.name}</div>
            <div class="sub ellipsis">
              {plural(r.songs.length, "canción", "canciones")}{#if r.missing > 0} · {r.missing} sin bajar{/if}
            </div>
          </div>
        </button>
      {/each}
    </div>
  </div>
  <div class="spacer-bottom"></div>
</div>

<style>
  .scroller {
    height: 100%;
    overflow-y: auto;
  }
  .actions {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
    margin: 18px 0 10px;
  }
  .list {
    margin: 0 -16px;
  }
  .row {
    width: 100%;
    display: flex;
    align-items: center;
    gap: 14px;
    padding: 8px 16px;
    text-align: left;
  }
  .icon {
    width: 56px;
    height: 56px;
    flex: none;
    display: grid;
    place-items: center;
    border-radius: calc(12px * var(--rs));
    background: rgb(var(--ov) / 0.06);
    color: var(--accent);
  }
  .text {
    min-width: 0;
  }
  .name {
    font-weight: 600;
  }
  .sub {
    font-size: 13px;
    color: var(--text-2);
    margin-top: 2px;
  }
</style>
