<script lang="ts">
  // Fondo de la ventana y variables de color segun los ajustes.
  import { fileUrl } from "../api";
  import { store } from "../store.svelte";

  const a = $derived(store.settings?.appearance);

  // Texto oscuro sobre acentos claros (todos los pastel lo son).
  function ink(hex: string) {
    const n = parseInt(hex.slice(1), 16);
    const l = (0.299 * ((n >> 16) & 255) + 0.587 * ((n >> 8) & 255) + 0.114 * (n & 255)) / 255;
    return l > 0.6 ? "#1a1a1a" : "#ffffff";
  }

  $effect(() => {
    if (!a) return;
    const root = document.documentElement.style;
    root.setProperty("--accent", a.accent);
    root.setProperty("--accent-ink", ink(a.accent));
    root.setProperty("--folder", a.folderColor);
  });

  const image = $derived(
    a?.background === "kiosk" ? "/kiosk-bg.webp" : a?.background === "image" && a.backgroundImage ? fileUrl(a.backgroundImage) : null,
  );
</script>

{#if a}
  <div class="bg" aria-hidden="true">
    {#if a.background === "color"}
      <div class="layer" style:background={a.backgroundColor}></div>
    {:else if image}
      <div class="layer img" style:background-image={`url("${image}")`} style:filter={a.blur ? `blur(${a.blur}px)` : undefined}></div>
    {:else}
      <!-- Vidrio: Windows desenfoca lo que hay detras; esto solo le da cuerpo. -->
      <div class="layer" style:background="rgba(14, 18, 22, 0.35)"></div>
    {/if}
    {#if a.background !== "color"}
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
  .img {
    inset: -40px;
    background-size: cover;
    background-position: center;
  }
</style>
