<script lang="ts">
  // Fondo, tema y paleta segun los ajustes. En el telefono no hay vidrio: el fondo es el
  // del tema o un color.
  import { applyTheme } from "@haze/ui/themes";
  import { store } from "../store.svelte";

  const a = $derived(store.settings?.appearance);

  $effect(() => {
    if (!a) return;
    applyTheme(a.theme, a.palette);
  });
</script>

<div class="bg" aria-hidden="true">
  {#if a?.background === "color"}
    <div class="layer" style:background={a.backgroundColor}></div>
  {:else}
    <div class="layer theme"></div>
  {/if}
</div>

<style>
  .bg {
    position: fixed;
    inset: 0;
    z-index: -1;
    overflow: hidden;
  }
  .layer {
    position: absolute;
    inset: 0;
  }
  .theme {
    inset: -40px;
    background-color: var(--bg);
    background-image: var(--bg-image);
  }
</style>
