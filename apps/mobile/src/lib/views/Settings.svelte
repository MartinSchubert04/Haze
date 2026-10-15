<script lang="ts">
  // Ajustes del telefono: acento, fondo y reproduccion. Home Assistant llega en la fase 2.
  import { ChevronLeft, Check, RefreshCw } from "@lucide/svelte";
  import { plural } from "@haze/ui/format";
  import { api } from "../api";
  import { store } from "../store.svelte";

  const ACCENTS = [
    "#ffffff", "#f5e0dc", "#f2cdcd", "#f5c2e7", "#cba6f7", "#f38ba8", "#eba0ac", "#fab387",
    "#f9e2af", "#a6e3a1", "#94e2d5", "#89dceb", "#74c7ec", "#89b4fa", "#b4befe",
  ];
  const BACKGROUNDS = ["#0f1720", "#000000", "#121212", "#1a1423", "#102019", "#1b1412", "#1d2328"];

  const s = $derived(store.settings!);
  const a = $derived(s.appearance);
</script>

<section class="layer">
  <div class="page">
    <button class="icon-btn back" aria-label="Volver" onclick={() => store.close()}><ChevronLeft size={26} /></button>
    <h1 class="page-title">Ajustes</h1>

    <h2>Color de acento</h2>
    <div class="swatches">
      {#each ACCENTS as c (c)}
        <button
          class="swatch"
          style:background={c}
          aria-label={c}
          onclick={() => store.saveSettings((x) => {
            x.appearance.accent = c;
            x.appearance.folderColor = c;
          })}
        >
          {#if a.accent === c}<Check size={16} color="#1a1a1a" strokeWidth={3} />{/if}
        </button>
      {/each}
    </div>

    <h2>Fondo</h2>
    <div class="choices">
      <button class="choice" class:on={a.background !== "color"} onclick={() => store.saveSettings((x) => (x.appearance.background = "haze"))}>
        <span class="preview haze"></span> Bruma
      </button>
      <button class="choice" class:on={a.background === "color"} onclick={() => store.saveSettings((x) => (x.appearance.background = "color"))}>
        <span class="preview" style:background={a.backgroundColor}></span> Color
      </button>
    </div>
    {#if a.background === "color"}
      <div class="swatches">
        {#each BACKGROUNDS as c (c)}
          <button class="swatch dark" style:background={c} aria-label={c} onclick={() => store.saveSettings((x) => (x.appearance.backgroundColor = c))}>
            {#if a.backgroundColor === c}<Check size={16} strokeWidth={3} />{/if}
          </button>
        {/each}
      </div>
    {:else}
      <label class="row">
        <span>Oscurecer</span>
        <input
          type="range"
          min="0"
          max="0.85"
          step="0.05"
          value={a.dim}
          onchange={(e) => {
            const v = Number(e.currentTarget.value);
            store.saveSettings((x) => (x.appearance.dim = v));
          }}
        />
      </label>
    {/if}

    <label class="row">
      <span>Teñir "Reproduciendo" con la tapa</span>
      <input
        type="checkbox"
        checked={a.tintFromArtwork}
        onchange={(e) => {
          const v = e.currentTarget.checked;
          store.saveSettings((x) => (x.appearance.tintFromArtwork = v));
        }}
      />
    </label>

    <h2>Reproducción</h2>
    <label class="row">
      <span>Retomar la última canción al abrir</span>
      <input
        type="checkbox"
        checked={s.playback.resume}
        onchange={(e) => {
          const v = e.currentTarget.checked;
          store.saveSettings((x) => (x.playback.resume = v));
        }}
      />
    </label>

    <h2>Biblioteca</h2>
    <div class="row">
      <span>{plural(store.songs.length, "canción", "canciones")} en el teléfono</span>
      <button class="pill" disabled={store.scanning} onclick={() => api.rescan()}>
        <RefreshCw size={15} class={store.scanning ? "spin" : ""} /> Volver a buscar
      </button>
    </div>

    <p class="version">Haze {store.version}</p>
  </div>
  <div class="spacer-bottom"></div>
</section>

<style>
  .layer {
    position: fixed;
    inset: 0;
    z-index: 10;
    overflow-y: auto;
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
  h2 {
    font-size: 13px;
    font-weight: 600;
    letter-spacing: 0.04em;
    text-transform: uppercase;
    color: var(--text-3);
    margin: 28px 0 12px;
  }
  .swatches {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(40px, 1fr));
    gap: 10px;
  }
  .swatch {
    aspect-ratio: 1;
    border-radius: 50%;
    display: grid;
    place-items: center;
    border: 1px solid var(--line-strong);
  }
  .swatch.dark {
    color: #fff;
  }
  .choices {
    display: flex;
    gap: 10px;
    margin-bottom: 14px;
  }
  .choice {
    flex: 1;
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 10px;
    border-radius: 14px;
    border: 1px solid var(--line);
  }
  .choice.on {
    border-color: var(--accent);
  }
  .preview {
    width: 32px;
    height: 32px;
    border-radius: 8px;
  }
  .preview.haze {
    background: var(--haze-bg);
  }
  .row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 16px;
    min-height: 52px;
  }
  input[type="checkbox"] {
    width: 22px;
    height: 22px;
    accent-color: var(--accent);
  }
  input[type="range"] {
    flex: 1;
    max-width: 50%;
    accent-color: var(--accent);
  }
  .version {
    margin-top: 32px;
    text-align: center;
    font-size: 13px;
    color: var(--text-3);
  }
</style>
