<script lang="ts">
  import { Star, FolderOpen } from "@lucide/svelte";
  import { openPath } from "@tauri-apps/plugin-opener";
  import { store } from "../store.svelte";
  import { folderName, normalizeSearch, plural } from "../format";
  import SongList from "../components/SongList.svelte";
  import ListHeader from "../components/ListHeader.svelte";

  let { path }: { path: string } = $props();

  const name = $derived(folderName(path));
  const songs = $derived(store.sortSongs(store.songs.filter((s) => s.folder === path)));
  const q = $derived(normalizeSearch(store.search.trim()));
  const shown = $derived(q ? songs.filter((s) => normalizeSearch(`${s.title} ${s.artist}`).includes(q)) : songs);
  const fav = $derived(store.settings?.downloadFolders.includes(path) ?? false);
  const sort = $derived(store.settings?.songSort ?? "title_asc");

  function toggleFav() {
    store.saveSettings((s) => {
      if (fav) {
        s.downloadFolders = s.downloadFolders.filter((p) => p !== path);
        if (s.defaultDownloadFolder === path) s.defaultDownloadFolder = s.downloadFolders[0] ?? null;
      } else {
        s.downloadFolders = [...s.downloadFolders, path];
        s.defaultDownloadFolder ??= path;
      }
    });
    store.toast(fav ? "Ya no es favorita" : "Favorita: aparece al bajar de YouTube");
  }
</script>

<SongList songs={shown} context={name} showFolder={false} alphabet={sort.startsWith("title")}>
  {#snippet header()}
    <ListHeader title={name} sub={`${plural(songs.length, "canción", "canciones")} · ${path}`} songs={shown} context={name}>
      {#snippet extra()}
        <button class="pill" class:on={fav} onclick={toggleFav} title="Las favoritas aparecen como destino al bajar de YouTube">
          <Star size={16} fill={fav ? "var(--accent)" : "none"} color={fav ? "var(--accent)" : "currentColor"} />
          {fav ? "Favorita" : "Marcar favorita"}
        </button>
        <button class="pill" onclick={() => openPath(path.replaceAll("/", "\\"))}><FolderOpen size={16} /> Abrir</button>
      {/snippet}
    </ListHeader>
  {/snippet}
</SongList>
