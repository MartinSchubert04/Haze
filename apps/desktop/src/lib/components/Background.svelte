<script lang="ts">
  // Fondo de la ventana, tema y paleta segun los ajustes.
  import { applyTheme } from "@haze/ui/themes";
  import { fileUrl } from "../api";
  import { store } from "../store.svelte";

  const a = $derived(store.settings?.appearance);
  // "haze" es la bruma de antes de los temas: ahora es el fondo del tema.
  const mode = $derived(a?.background === "haze" ? "theme" : a?.background);

  $effect(() => {
    if (!a) return;
    applyTheme(a.theme, a.palette);
    const root = document.documentElement.style;
    // Sin color propio, las carpetas van con el acento de la paleta.
    if (a.folderColor) root.setProperty("--folder", a.folderColor);
    else root.removeProperty("--folder");
  });

  const image = $derived(mode === "image" && a?.backgroundImage ? fileUrl(a.backgroundImage) : null);
</script>

{#if a}
  <div class="bg" aria-hidden="true">
    {#if mode === "color"}
      <div class="layer" style:background={a.backgroundColor}></div>
    {:else if image}
      <div class="layer img" style:background-image={`url("${image}")`} style:filter={a.blur ? `blur(${a.blur}px)` : undefined}></div>
    {:else if mode === "glass"}
      <!-- Vidrio: Windows desenfoca lo que hay detras; esto solo le da cuerpo. -->
      <div class="layer tint"></div>
    {:else}
      <div class="layer theme" style:filter={a.blur ? `blur(${a.blur}px)` : undefined}></div>
    {/if}
    <!-- El tema ya trae su fondo pensado para que el texto se lea: oscurecer es para
         lo que elige uno (imagen o vidrio). -->
    {#if image || mode === "glass"}
      <div class="layer" style:background={`rgba(6, 8, 10, ${a.dim})`}></div>
    {/if}
  </div>
{/if}

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
  .tint {
    background: color-mix(in srgb, var(--bg) 35%, transparent);
  }
  .img {
    inset: -40px;
    background-size: cover;
    background-position: center;
  }
</style>
