<script lang="ts">
  import { menu } from "../menu.svelte";

  let el = $state<HTMLDivElement>();
  let pos = $state({ x: 0, y: 0 });

  // Que no se salga de la ventana.
  $effect(() => {
    const m = menu.open;
    if (!m || !el) return;
    const r = el.getBoundingClientRect();
    pos = {
      x: Math.min(m.x, window.innerWidth - r.width - 8),
      y: Math.min(m.y, window.innerHeight - r.height - 8),
    };
  });
</script>

<svelte:window
  onkeydown={(e) => e.key === "Escape" && menu.close()}
  onblur={() => menu.close()}
  onresize={() => menu.close()}
/>

{#if menu.open}
  <!-- svelte-ignore a11y_no_static_element_interactions, a11y_click_events_have_key_events -->
  <div class="backdrop" onclick={() => menu.close()} oncontextmenu={(e) => { e.preventDefault(); menu.close(); }}></div>
  <div class="menu" bind:this={el} style:left="{pos.x || menu.open.x}px" style:top="{pos.y || menu.open.y}px" role="menu">
    {#each menu.open.items as it}
      {#if it === "sep"}
        <div class="sep"></div>
      {:else}
        <button
          role="menuitem"
          class:danger={it.danger}
          onclick={() => {
            menu.close();
            it.action();
          }}
        >
          {#if it.icon}<it.icon size={15} />{/if}
          <span>{it.label}</span>
        </button>
      {/if}
    {/each}
  </div>
{/if}

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    z-index: 90;
  }
  .menu {
    position: fixed;
    z-index: 91;
    min-width: 220px;
    padding: 6px;
    border-radius: calc(12px * var(--rs));
    background: var(--pop);
    backdrop-filter: var(--blur);
    border: var(--border) solid var(--line-strong);
    box-shadow: var(--pop-shadow);
    animation: pop 0.12s ease-out;
  }
  @keyframes pop {
    from {
      opacity: 0;
      transform: scale(0.97);
    }
  }
  button {
    display: flex;
    align-items: center;
    gap: 10px;
    width: 100%;
    height: 34px;
    padding: 0 10px;
    border-radius: calc(8px * var(--rs));
    text-align: left;
    font-size: 13.5px;
    color: var(--text);
  }
  button :global(svg) {
    color: var(--text-2);
  }
  button:hover {
    background: rgb(var(--ov) / 0.08);
  }
  .danger {
    color: var(--danger);
  }
  .sep {
    height: 1px;
    margin: 5px 6px;
    background: var(--line);
  }
</style>
