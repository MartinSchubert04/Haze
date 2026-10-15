<script lang="ts">
  import { Folder } from "@lucide/svelte";
  import { plural } from "@haze/ui/format";
  import { store } from "../store.svelte";
  import Cover from "../components/Cover.svelte";
</script>

<div class="scroller">
  <div class="page">
    <h1 class="page-title">Carpetas</h1>
    <div class="page-sub">{plural(store.folders.length, "carpeta", "carpetas")}</div>

    <div class="list">
      {#each store.folders as f (f.path)}
        <button class="folder press" onclick={() => store.open({ kind: "folder", path: f.path })}>
          {#if f.coverSong}
            <Cover song={f.coverSong} size={56} radius={12} />
          {:else}
            <div class="icon"><Folder size={26} /></div>
          {/if}
          <div class="text">
            <div class="name ellipsis">{f.name}</div>
            <div class="sub ellipsis">{plural(f.songs.length, "canción", "canciones")} · {f.path || "/"}</div>
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
  .list {
    margin: 0 -16px;
  }
  .folder {
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
    border-radius: 12px;
    background: rgba(255, 255, 255, 0.06);
    color: var(--folder);
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
