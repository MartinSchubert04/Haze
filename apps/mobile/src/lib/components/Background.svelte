<script lang="ts">
  // Fondo y color de acento segun los ajustes. En el telefono no hay vidrio: la bruma
  // ocupa su lugar.
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
</script>

<div class="bg" aria-hidden="true">
  {#if a?.background === "color"}
    <div class="layer" style:background={a.backgroundColor}></div>
  {:else}
    <div class="layer haze"></div>
    <div class="layer" style:background={`rgba(6, 8, 10, ${a?.dim ?? 0.45})`}></div>
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
  .haze {
    inset: -40px;
    background: var(--haze-bg);
  }
</style>
