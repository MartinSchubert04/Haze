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
    <button class="tab" class:on={store.tab === t.id} onclick={() => store.setTab(t.id)}>
      <span class="ind"><t.icon size={22} strokeWidth={store.tab === t.id ? 2.2 : 1.8} /></span>
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
  /* Al tocar se marca una pastilla detras del icono, como en Material 3. */
  .ind {
    display: grid;
    place-items: center;
    width: 56px;
    height: 30px;
    border-radius: calc(15px * var(--rs));
    transition:
      background-color var(--release),
      transform 0.4s cubic-bezier(0.2, 0.8, 0.2, 1) 0.06s;
  }
  .tab:global(.pressed) .ind {
    background-color: var(--press-bg);
    transform: scale(0.94);
    transition:
      background-color 0.05s,
      transform 0.08s;
  }
</style>
