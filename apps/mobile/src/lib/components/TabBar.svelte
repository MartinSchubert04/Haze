<script lang="ts">
  import { Music, Folder, ListMusic, TvMinimalPlay } from "@lucide/svelte";
  import { store, type Tab } from "../store.svelte";

  const tabs: { id: Tab; label: string; icon: typeof Music }[] = [
    { id: "songs", label: "Canciones", icon: Music },
    { id: "folders", label: "Carpetas", icon: Folder },
    { id: "youtube", label: "YouTube", icon: TvMinimalPlay },
    { id: "queue", label: "Cola", icon: ListMusic },
  ];
</script>

<nav>
  {#each tabs as t (t.id)}
    <button class:on={store.tab === t.id} onclick={() => store.setTab(t.id)}>
      <t.icon size={22} strokeWidth={store.tab === t.id ? 2.2 : 1.8} />
      <span>{t.label}</span>
    </button>
  {/each}
</nav>

<style>
  nav {
    height: var(--tabbar);
    display: flex;
  }
  button {
    flex: 1;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 3px;
    color: var(--text-3);
    font-size: 11.5px;
    font-weight: 550;
  }
  button.on {
    color: var(--accent);
  }
</style>
