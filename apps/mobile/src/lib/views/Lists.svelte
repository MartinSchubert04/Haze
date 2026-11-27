<script lang="ts">
  import { ListMusic, Plus } from "@lucide/svelte";
  import { plural, songCount } from "../format";
  import { sortLists } from "@haze/ui/library";
  import type { List } from "../api";
  import { store } from "../store.svelte";
  import { longpress } from "../longpress";
  import Cover from "../components/Cover.svelte";
  import Select from "../components/Select.svelte";
  import { LIST_SORTS } from "../sorts";

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
    <h1 class="page-title">Lists</h1>
    <div class="page-sub">
      {plural(store.lists.length, "list", "lists")}{#if store.lists.length} · long-press one to rename or delete it{/if}
    </div>
    <div class="actions">
      <button class="pill primary" onclick={() => store.open({ kind: "listName" })}><Plus size={16} /> New list</button>
      {#if store.lists.length > 1}
        <Select label="Sort" value={sort} options={LIST_SORTS} onchange={(k) => store.saveSettings((x) => (x.listSort = k))} />
      {/if}
    </div>

    {#if !store.lists.length}
      <div class="empty">
        <h3>No lists yet</h3>
        <p>Create one here, or long-press a song and pick "Add to list…". Lists are shared with your PC when you sync at home.</p>
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
              {songCount(r.songs.length)}{#if r.missing > 0} · {r.missing} not downloaded{/if}
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
