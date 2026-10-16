<script lang="ts">
  import { ChevronLeft, Play, Shuffle } from "@lucide/svelte";
  import { folderName, plural } from "@haze/ui/format";
  import { api } from "../api";
  import { store } from "../store.svelte";
  import SongList from "../components/SongList.svelte";

  let { path }: { path: string } = $props();

  const name = $derived(folderName(path) || "Almacenamiento");
  const songs = $derived(store.sortSongs(store.folders.find((f) => f.path === path)?.songs ?? []));

  async function play(shuffle: boolean) {
    if (store.player.shuffle !== shuffle) await api.control({ type: "shuffle", value: shuffle });
    store.play(songs, shuffle ? Math.floor(Math.random() * songs.length) : 0, name);
  }
</script>

<section class="layer">
  {#snippet header()}
    <div class="page">
      <button class="icon-btn back" aria-label="Volver" onclick={() => store.close()}><ChevronLeft size={26} /></button>
      <h1 class="page-title">{name}</h1>
      <div class="page-sub">{plural(songs.length, "canción", "canciones")} · {path || "/"}</div>
      <div class="actions">
        <button class="pill primary" disabled={!songs.length} onclick={() => play(false)}><Play size={16} fill="currentColor" /> Reproducir</button>
        <button class="pill" disabled={!songs.length} onclick={() => play(true)}><Shuffle size={16} /> Aleatorio</button>
      </div>
    </div>
  {/snippet}
  <SongList {songs} context={name} {header} onrefresh={() => api.rescan()} />
</section>

<style>
  .layer {
    position: fixed;
    inset: 0;
    z-index: 10;
    background: #12171b;
    animation: in 0.2s ease-out;
  }
  @keyframes in {
    from {
      transform: translateX(24px);
      opacity: 0;
    }
  }
  .back {
    margin-left: -10px;
  }
  .page-sub {
    margin-bottom: 14px;
  }
  .actions {
    display: flex;
    gap: 10px;
  }
</style>
