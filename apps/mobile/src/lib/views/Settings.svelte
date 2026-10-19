<script lang="ts">
  // Ajustes del telefono: acento, fondo, reproduccion y Home Assistant.
  import { ChevronLeft, ChevronDown, Check, RefreshCw, Download, Eye } from "@lucide/svelte";
  import { folderName, plural } from "@haze/ui/format";
  import { api } from "../api";
  import { store } from "../store.svelte";

  const ACCENTS = [
    "#ffffff", "#f5e0dc", "#f2cdcd", "#f5c2e7", "#cba6f7", "#f38ba8", "#eba0ac", "#fab387",
    "#f9e2af", "#a6e3a1", "#94e2d5", "#89dceb", "#74c7ec", "#89b4fa", "#b4befe",
  ];
  const BACKGROUNDS = ["#0f1720", "#000000", "#121212", "#1a1423", "#102019", "#1b1412", "#1d2328"];

  const s = $derived(store.settings!);
  const a = $derived(s.appearance);

  // Home Assistant se edita en un borrador y se guarda con un boton: si no, cada letra
  // reconectaria al broker.
  let ha = $state({ ...store.settings!.ha });
  const haDirty = $derived(JSON.stringify(ha) !== JSON.stringify(s.ha));
  const haLabel = $derived(
    ({ connected: "Conectado", error: "Sin conexión", network: "Falta permiso" } as Record<string, string>)[store.haStatus] ??
      "Conectando…",
  );

  // Home Assistant se muestra plegado: casi nunca se toca.
  let haOpen = $state(false);

  let checking = $state(false);
  let upToDate = $state(false);

  async function checkUpdate() {
    checking = true;
    upToDate = false;
    try {
      upToDate = !(await api.updateCheck()) && !store.update;
    } catch (e) {
      store.toast(String(e), "error");
    } finally {
      checking = false;
    }
  }

  async function install() {
    try {
      if (!(await api.updateInstall())) store.toast('Permití "Instalar apps desconocidas" para Haze y tocá de nuevo');
    } catch (e) {
      store.toast(String(e), "error");
    }
  }

  async function saveHa() {
    await store.saveSettings((x) => (x.ha = { ...ha, port: Number(ha.port) || 1883 }));
    // Despues de guardar: si dan el permiso, Rust reconecta con lo nuevo.
    if (ha.enabled) await api.requestLocalNetwork();
  }
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

    <button class="fold" aria-expanded={haOpen} onclick={() => (haOpen = !haOpen)}>
      <h2>Home Assistant</h2>
      {#if s.ha.enabled}
        <span class="status" class:ok={store.haStatus === "connected"}>{haLabel}</span>
      {/if}
      <span class="chev" class:open={haOpen}><ChevronDown size={18} /></span>
    </button>
    {#if haOpen}
    <p class="hint">Por MQTT. Las entidades aparecen solas en HA, en el dispositivo "{s.ha.deviceName}".</p>
    <label class="row">
      <span>Conectar con Home Assistant</span>
      {#if s.ha.enabled}
        <span class="status" class:ok={store.haStatus === "connected"}>{haLabel}</span>
      {/if}
      <input type="checkbox" bind:checked={ha.enabled} />
    </label>
    <div class="form" class:off={!ha.enabled}>
      <label>Servidor MQTT<input type="text" inputmode="url" bind:value={ha.host} placeholder="192.168.1.10" /></label>
      <label>Puerto<input type="number" inputmode="numeric" bind:value={ha.port} /></label>
      <label>Usuario<input type="text" bind:value={ha.username} autocomplete="off" autocapitalize="off" /></label>
      <label>Contraseña<input type="password" bind:value={ha.password} autocomplete="off" /></label>
      <label>
        Nombre del dispositivo<input type="text" bind:value={ha.deviceName} />
        <span class="hint">Si usás Haze en más de un aparato, poné un nombre distinto en cada uno.</span>
      </label>
      {#if s.ha.enabled && store.haStatus === "network"}
        <span class="hint">Android pide permiso para que Haze llegue a la red local (lo muestra como "dispositivos cercanos").</span>
        <button class="pill save" onclick={() => api.requestLocalNetwork()}>Dar permiso</button>
      {/if}
      <button class="pill primary save" disabled={!haDirty} onclick={saveHa}>Guardar</button>
    </div>
    {/if}

    <h2>Biblioteca</h2>
    <div class="row">
      <span>{plural(store.songs.length, "canción", "canciones")} en el teléfono</span>
      <button class="pill" disabled={store.scanning} onclick={() => api.rescan()}>
        <RefreshCw size={15} class={store.scanning ? "spin" : ""} /> Volver a buscar
      </button>
    </div>
    {#if s.hiddenFolders?.length}
      <p class="hint">Carpetas quitadas de la biblioteca</p>
      {#each s.hiddenFolders as path (path)}
        <div class="row hidden-folder">
          <div class="folder-text">
            <div class="ellipsis">{folderName(path) || "Almacenamiento"}</div>
            <div class="hint ellipsis">{path || "/"}</div>
          </div>
          <button class="pill" onclick={() => store.showFolder(path)}><Eye size={15} /> Mostrar</button>
        </div>
      {/each}
    {/if}

    <h2>Actualizaciones</h2>
    <div class="row">
      <span>Versión {store.version}</span>
      {#if store.update?.status === "ready"}
        <button class="pill primary" onclick={install}>Actualizar ahora</button>
      {:else}
        <button class="pill" onclick={checkUpdate} disabled={checking || !!store.update}>
          <RefreshCw size={15} class={checking || store.update ? "spin" : ""} /> Buscar
        </button>
      {/if}
    </div>
    {#if store.update}
      <div class="update">
        <Download size={18} />
        <span>
          {store.update.status === "ready"
            ? `Haze ${store.update.version} está lista. Se instala sola al salir de la app sin música sonando.`
            : `Bajando Haze ${store.update.version}… ${Math.round(store.update.progress * 100)}%`}
        </span>
      </div>
    {:else if upToDate}
      <div class="update"><Check size={18} /> Estás en la última versión</div>
    {/if}
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
  .fold {
    width: 100%;
    display: flex;
    align-items: center;
    gap: 10px;
    margin: 28px 0 12px;
    text-align: left;
    color: var(--text-3);
  }
  .fold h2 {
    flex: 1;
    margin: 0;
  }
  .fold .status {
    font-size: 13px;
    color: var(--text-3);
  }
  .fold .status.ok {
    color: #a6e3a1;
  }
  .chev {
    display: grid;
    transition: transform 0.2s;
  }
  .chev.open {
    transform: rotate(180deg);
  }
  .hidden-folder {
    min-height: 56px;
  }
  .folder-text {
    min-width: 0;
  }
  .folder-text .hint {
    margin: 2px 0 0;
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
  .hint {
    font-size: 13px;
    color: var(--text-3);
    margin: 0 0 4px;
  }
  .row .status {
    margin-left: auto;
    font-size: 13px;
    color: var(--text-3);
  }
  .row .status.ok {
    color: #a6e3a1;
  }
  .form {
    display: flex;
    flex-direction: column;
    gap: 14px;
    margin-top: 6px;
  }
  .form.off {
    opacity: 0.45;
    pointer-events: none;
  }
  .form label {
    display: flex;
    flex-direction: column;
    gap: 6px;
    font-size: 14px;
    color: var(--text-2);
  }
  .form input {
    height: 46px;
    padding: 0 14px;
    border-radius: 12px;
    border: 1px solid var(--line);
    background: rgba(255, 255, 255, 0.05);
    color: var(--text);
    font: inherit;
    font-size: 16px;
  }
  .form input:focus {
    outline: none;
    border-color: var(--accent);
  }
  .save {
    align-self: flex-end;
  }
  .update {
    display: flex;
    align-items: center;
    gap: 10px;
    font-size: 13.5px;
    color: var(--text-2);
  }
</style>
