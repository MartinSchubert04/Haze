<script lang="ts">
  import { Star, Check } from "@lucide/svelte";
  import { ArrowUpDown } from "@lucide/svelte";
  import { store, type Folder } from "../store.svelte";
  import { menu } from "../menu.svelte";
  import { normalizeSearch, plural, sortKey } from "../format";
  import Cover from "../components/Cover.svelte";

  const q = $derived(normalizeSearch(store.search.trim()));
  const sort = $derived(store.settings?.folderSort ?? "name_asc");
  const favs = $derived(new Set(store.settings?.downloadFolders ?? []));

  const list = $derived.by(() => {
    const l = q ? store.folders.filter((f) => normalizeSearch(f.name).includes(q)) : [...store.folders];
    switch (sort) {
      case "name_desc":
        return l.sort((a, b) => sortKey(b.name).localeCompare(sortKey(a.name)));
      case "count_desc":
        return l.sort((a, b) => b.songs.length - a.songs.length);
      case "count_asc":
        return l.sort((a, b) => a.songs.length - b.songs.length);
      default:
        return l.sort((a, b) => sortKey(a.name).localeCompare(sortKey(b.name)));
    }
  });

  function sortMenu(e: MouseEvent) {
    const opts: [string, string][] = [
      ["name_asc", "Nombre (A-Z)"],
      ["name_desc", "Nombre (Z-A)"],
      ["count_desc", "Más canciones"],
      ["count_asc", "Menos canciones"],
    ];
    menu.show(e, opts.map(([k, label]) => ({ label, icon: k === sort ? Check : undefined, action: () => store.saveSettings((s) => (s.folderSort = k)) })));
  }

  // Hasta 4 tapas distintas para el mosaico de la carpeta.
  function covers(f: Folder) {
    const seen = new Set<string>();
    const out = [];
    for (const s of f.songs) {
      if (!s.hasCover) continue;
      const key = s.album || s.path;
      if (seen.has(key)) continue;
      seen.add(key);
      out.push(s);
      if (out.length === 4) break;
    }
    return out;
  }
</script>

<div class="scroll">
  <div class="page">
    <div class="top">
      <div>
        <h1 class="page-title">Carpetas</h1>
        <div class="page-sub">{plural(list.length, "carpeta", "carpetas")}</div>
      </div>
      <button class="icon-btn" title="Ordenar" onclick={sortMenu}><ArrowUpDown size={17} /></button>
    </div>

    <div class="grid">
      {#each list as f (f.path)}
        {@const cs = covers(f)}
        <button class="card" title={f.path} onclick={() => store.go({ name: "folder", path: f.path })}>
          <div class="art" class:mosaic={cs.length >= 4}>
            {#if cs.length >= 4}
              {#each cs as s}<Cover song={s} size="100%" radius={0} />{/each}
            {:else}
              <Cover song={cs[0] ?? null} size="100%" radius={0} />
            {/if}
            {#if favs.has(f.path)}<span class="fav" title="Carpeta favorita"><Star size={13} fill="currentColor" /></span>{/if}
          </div>
          <div class="name ellipsis">{f.name}</div>
          <div class="count">{plural(f.songs.length, "canción", "canciones")}</div>
        </button>
      {/each}
    </div>
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
  .top .icon-btn {
    margin-bottom: 16px;
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
    background: rgba(255, 255, 255, 0.07);
  }
  .card:active {
    transform: scale(0.985);
  }
  .art {
    position: relative;
    aspect-ratio: 1;
    border-radius: 12px;
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
  .fav {
    position: absolute;
    top: 8px;
    right: 8px;
    width: 24px;
    height: 24px;
    border-radius: 50%;
    display: grid;
    place-items: center;
    background: rgba(0, 0, 0, 0.55);
    color: var(--accent);
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
