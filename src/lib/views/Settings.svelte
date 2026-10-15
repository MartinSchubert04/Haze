<script lang="ts">
  import { open } from "@tauri-apps/plugin-dialog";
  import { enable, disable, isEnabled } from "@tauri-apps/plugin-autostart";
  import { openUrl } from "@tauri-apps/plugin-opener";
  import { FolderPlus, Trash2, RefreshCw, Star, ImagePlus, Check, Download } from "@lucide/svelte";
  import { api, fileUrl } from "../api";
  import { store } from "../store.svelte";
  import { folderName, plural } from "../format";

  const ACCENTS = [
    "#ffffff", "#f5e0dc", "#f2cdcd", "#f5c2e7", "#cba6f7", "#f38ba8", "#eba0ac", "#fab387",
    "#f9e2af", "#a6e3a1", "#94e2d5", "#89dceb", "#74c7ec", "#89b4fa", "#b4befe",
  ];
  const BACKGROUNDS = ["#0f1720", "#000000", "#121212", "#1a1423", "#102019", "#1b1412", "#1d2328"];
  const MODES: [string, string, string][] = [
    ["glass", "Vidrio", "Desenfoca lo que hay detrás de la ventana"],
    ["kiosk", "Kiosco", "El fondo del kiosco de casa"],
    ["image", "Imagen", "Una imagen tuya"],
    ["color", "Color", "Un color liso"],
  ];

  const s = $derived(store.settings!);
  let autostart = $state(false);
  isEnabled().then((v) => (autostart = v), () => {});

  // Home Assistant se edita en un borrador y se guarda con un boton: si no, cada letra
  // que se tipea reconectaria MQTT.
  let ha = $state({ ...store.settings!.ha });
  const haDirty = $derived(JSON.stringify(ha) !== JSON.stringify(s.ha));

  async function addSource() {
    const dir = await open({ directory: true, multiple: true, title: "Carpetas con música" });
    const dirs = (Array.isArray(dir) ? dir : dir ? [dir] : []).map((d) => d.replaceAll("\\", "/"));
    if (dirs.length) store.saveSettings((x) => (x.sources = [...new Set([...x.sources, ...dirs])]));
  }

  async function addFav() {
    const dir = await open({ directory: true, title: "Carpeta para descargas" });
    if (typeof dir !== "string") return;
    const d = dir.replaceAll("\\", "/");
    store.saveSettings((x) => {
      if (!x.downloadFolders.includes(d)) x.downloadFolders = [...x.downloadFolders, d];
      x.defaultDownloadFolder ??= d;
    });
  }

  async function pickImage() {
    const f = await open({ title: "Imagen de fondo", filters: [{ name: "Imágenes", extensions: ["jpg", "jpeg", "png", "webp", "gif"] }] });
    if (typeof f === "string") store.saveSettings((x) => ((x.appearance.backgroundImage = f), (x.appearance.background = "image")));
  }

  async function toggleAutostart() {
    try {
      if (autostart) await disable();
      else await enable();
      autostart = await isEnabled();
    } catch (e) {
      store.toast(String(e), "error");
    }
  }

  const count = (dir: string) => store.songs.filter((x) => x.path.toLowerCase().startsWith(dir.toLowerCase() + "/")).length;
</script>

<div class="scroll">
  <div class="page">
    <h1 class="page-title">Ajustes</h1>
    <div class="page-sub">Versión {store.version}</div>

    {#if store.update}
      <button class="update" onclick={() => openUrl(store.update!.url)}>
        <Download size={18} />
        <div>
          <div>Hay una versión nueva: {store.update.version}</div>
          <div class="hint">Tocá para abrir la página de descarga.</div>
        </div>
      </button>
    {/if}

    <!-- ------------------------------------------------------------ biblioteca -->
    <section>
      <div class="sec-head">
        <div>
          <h2>Biblioteca</h2>
          <p>Carpetas de donde salen tus canciones. Se revisan también sus subcarpetas.</p>
        </div>
        <button class="pill" onclick={() => api.rescan()} disabled={store.scanning}>
          <RefreshCw size={15} class={store.scanning ? "spin" : ""} /> {store.scanning ? "Escaneando…" : "Volver a escanear"}
        </button>
      </div>
      <div class="rows">
        {#each s.sources as src}
          <div class="item">
            <div class="grow">
              <div class="ellipsis">{folderName(src)}</div>
              <div class="hint ellipsis">{src} · {plural(count(src), "canción", "canciones")}</div>
            </div>
            <button class="icon-btn" title="Quitar" onclick={() => store.saveSettings((x) => (x.sources = x.sources.filter((p) => p !== src)))}>
              <Trash2 size={16} />
            </button>
          </div>
        {/each}
        <button class="add" onclick={addSource}><FolderPlus size={16} /> Agregar carpeta</button>
      </div>
    </section>

    <!-- ------------------------------------------------------------ descargas -->
    <section>
      <h2>Carpetas favoritas</h2>
      <p>Aparecen como destino al bajar de YouTube y como accesos en la barra lateral. La estrella marca la que se propone primero.</p>
      <div class="rows">
        {#each s.downloadFolders as d}
          <div class="item">
            <button
              class="icon-btn"
              class:on={s.defaultDownloadFolder === d}
              title="Usar por defecto"
              onclick={() => store.saveSettings((x) => (x.defaultDownloadFolder = d))}
            >
              <Star size={16} fill={s.defaultDownloadFolder === d ? "currentColor" : "none"} />
            </button>
            <div class="grow">
              <div class="ellipsis">{folderName(d)}</div>
              <div class="hint ellipsis">{d}</div>
            </div>
            <button
              class="icon-btn"
              title="Quitar de favoritas"
              onclick={() =>
                store.saveSettings((x) => {
                  x.downloadFolders = x.downloadFolders.filter((p) => p !== d);
                  if (x.defaultDownloadFolder === d) x.defaultDownloadFolder = x.downloadFolders[0] ?? null;
                })}
            >
              <Trash2 size={16} />
            </button>
          </div>
        {/each}
        <button class="add" onclick={addFav}><FolderPlus size={16} /> Agregar carpeta favorita</button>
      </div>
    </section>

    <!-- ------------------------------------------------------------ apariencia -->
    <section>
      <h2>Apariencia</h2>

      <div class="field">
        <div class="name">Color de acento</div>
        <div class="swatches">
          {#each ACCENTS as c}
            <button class="sw" style:background={c} title={c} onclick={() => store.saveSettings((x) => (x.appearance.accent = c))}>
              {#if s.appearance.accent === c}<Check size={14} color="#111" />{/if}
            </button>
          {/each}
        </div>
      </div>

      <div class="field">
        <div class="name">Fondo</div>
        <div class="modes">
          {#each MODES as [k, label, hint]}
            <button class="mode" class:sel={s.appearance.background === k} onclick={() => (k === "image" && !s.appearance.backgroundImage ? pickImage() : store.saveSettings((x) => (x.appearance.background = k as never)))}>
              <div class="prev {k}" style:background-color={k === "color" ? s.appearance.backgroundColor : undefined} style:background-image={k === "image" && s.appearance.backgroundImage ? `url("${fileUrl(s.appearance.backgroundImage)}")` : undefined}></div>
              <div class="ml">{label}</div>
              <div class="hint">{hint}</div>
            </button>
          {/each}
        </div>
      </div>

      {#if s.appearance.background === "color"}
        <div class="field">
          <div class="name">Color de fondo</div>
          <div class="swatches">
            {#each BACKGROUNDS as c}
              <button class="sw dark" style:background={c} onclick={() => store.saveSettings((x) => (x.appearance.backgroundColor = c))}>
                {#if s.appearance.backgroundColor === c}<Check size={14} />{/if}
              </button>
            {/each}
          </div>
        </div>
      {/if}

      {#if s.appearance.background === "image"}
        <div class="field">
          <div class="name">Imagen</div>
          <button class="pill" onclick={pickImage}><ImagePlus size={15} /> Cambiar imagen</button>
        </div>
      {/if}

      {#if s.appearance.background !== "color"}
        <div class="field">
          <div class="name">Oscurecer <span class="val">{Math.round(s.appearance.dim * 100)}%</span></div>
          <input type="range" min="0" max="0.9" step="0.01" value={s.appearance.dim} onchange={(e) => store.saveSettings((x) => (x.appearance.dim = +e.currentTarget.value))} oninput={(e) => store.settings && (store.settings.appearance.dim = +e.currentTarget.value)} />
        </div>
      {/if}
      {#if s.appearance.background === "image" || s.appearance.background === "kiosk"}
        <div class="field">
          <div class="name">Desenfoque <span class="val">{Math.round(s.appearance.blur)} px</span></div>
          <input type="range" min="0" max="40" step="1" value={s.appearance.blur} onchange={(e) => store.saveSettings((x) => (x.appearance.blur = +e.currentTarget.value))} oninput={(e) => store.settings && (store.settings.appearance.blur = +e.currentTarget.value)} />
        </div>
      {/if}

      <div class="field">
        <div class="name">Color de las carpetas</div>
        <div class="swatches">
          {#each ACCENTS as c}
            <button class="sw" style:background={c} onclick={() => store.saveSettings((x) => (x.appearance.folderColor = c))}>
              {#if s.appearance.folderColor === c}<Check size={14} color="#111" />{/if}
            </button>
          {/each}
        </div>
      </div>

      <label class="toggle">
        <div>
          <div>Teñir con la carátula</div>
          <div class="hint">La pantalla de reproducción toma el color de la tapa.</div>
        </div>
        <input type="checkbox" checked={s.appearance.tintFromArtwork} onchange={(e) => store.saveSettings((x) => (x.appearance.tintFromArtwork = e.currentTarget.checked))} />
      </label>
    </section>

    <!-- ------------------------------------------------------------ comportamiento -->
    <section>
      <h2>Ventana y reproducción</h2>
      <label class="toggle">
        <div>
          <div>Iniciar con Windows</div>
          <div class="hint">Arranca escondida en la bandeja, lista para Home Assistant y las teclas multimedia.</div>
        </div>
        <input type="checkbox" checked={autostart} onchange={toggleAutostart} />
      </label>
      <label class="toggle">
        <div>
          <div>Cerrar a la bandeja</div>
          <div class="hint">La X esconde la ventana y la música sigue. Para salir: clic derecho en el ícono de la bandeja.</div>
        </div>
        <input type="checkbox" checked={s.behavior.closeToTray} onchange={(e) => store.saveSettings((x) => (x.behavior.closeToTray = e.currentTarget.checked))} />
      </label>
      <label class="toggle">
        <div>
          <div>Mini reproductor siempre arriba</div>
          <div class="hint">Se aplica la próxima vez que lo abras.</div>
        </div>
        <input type="checkbox" checked={s.behavior.miniOnTop} onchange={(e) => store.saveSettings((x) => (x.behavior.miniOnTop = e.currentTarget.checked))} />
      </label>
      <label class="toggle">
        <div>
          <div>Retomar donde quedó</div>
          <div class="hint">Al abrir, carga la última canción en pausa, en el mismo segundo.</div>
        </div>
        <input type="checkbox" checked={s.playback.resume} onchange={(e) => store.saveSettings((x) => (x.playback.resume = e.currentTarget.checked))} />
      </label>
    </section>

    <!-- ------------------------------------------------------------ home assistant -->
    <section>
      <div class="sec-head">
        <div>
          <h2>Home Assistant</h2>
          <p>Por MQTT. Las entidades aparecen solas en HA, en el dispositivo "{s.ha.deviceName}".</p>
        </div>
        {#if s.ha.enabled}
          <span class="status" class:ok={store.haStatus === "connected"}>
            {store.haStatus === "connected" ? "Conectado" : store.haStatus === "error" ? "Sin conexión" : "Conectando…"}
          </span>
        {/if}
      </div>
      <label class="toggle">
        <div>Conectar con Home Assistant</div>
        <input type="checkbox" bind:checked={ha.enabled} />
      </label>
      <div class="form" class:off={!ha.enabled}>
        <label>Servidor MQTT<input type="text" bind:value={ha.host} placeholder="192.168.0.100" /></label>
        <label>Puerto<input type="number" bind:value={ha.port} /></label>
        <label>Usuario<input type="text" bind:value={ha.username} autocomplete="off" /></label>
        <label>Contraseña<input type="password" bind:value={ha.password} autocomplete="off" /></label>
        <label class="wide">Nombre del dispositivo<input type="text" bind:value={ha.deviceName} /></label>
      </div>
      <div class="save">
        <button class="pill primary" disabled={!haDirty} onclick={() => store.saveSettings((x) => (x.ha = { ...ha, port: Number(ha.port) || 1883 }))}>
          Guardar
        </button>
      </div>
    </section>

    <section>
      <h2>Salir</h2>
      <p>Cierra la app del todo, incluida la música de fondo.</p>
      <button class="pill" onclick={() => api.quit()}>Salir de Reproductor</button>
    </section>
  </div>
</div>

<style>
  .scroll {
    height: 100%;
    overflow-y: auto;
  }
  .page {
    max-width: 860px;
  }
  .update {
    display: flex;
    align-items: center;
    gap: 14px;
    width: 100%;
    text-align: left;
    padding: 14px 18px;
    border-radius: var(--radius-lg);
    background: color-mix(in srgb, var(--accent) 16%, transparent);
    border: 1px solid color-mix(in srgb, var(--accent) 45%, transparent);
    color: var(--accent);
    font-weight: 580;
  }
  section {
    margin-top: 18px;
    padding: 22px;
    border-radius: var(--radius-lg);
    background: var(--tile);
  }
  h2 {
    margin: 0 0 4px;
    font-size: 17px;
    font-weight: 620;
  }
  section > p,
  .sec-head p {
    margin: 0 0 14px;
    color: var(--text-2);
    font-size: 13px;
    line-height: 1.45;
  }
  .sec-head {
    display: flex;
    justify-content: space-between;
    gap: 16px;
    align-items: flex-start;
  }
  .rows {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .item {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 8px 8px 8px 14px;
    border-radius: var(--radius);
    background: rgba(255, 255, 255, 0.04);
  }
  .grow {
    flex: 1;
    min-width: 0;
  }
  .hint {
    color: var(--text-3);
    font-size: 12px;
    margin-top: 2px;
  }
  .add {
    display: flex;
    align-items: center;
    gap: 8px;
    height: 42px;
    padding: 0 14px;
    border-radius: var(--radius);
    border: 1px dashed var(--line-strong);
    color: var(--text-2);
    margin-top: 4px;
  }
  .add:hover {
    color: var(--text);
    background: rgba(255, 255, 255, 0.04);
  }
  .field {
    margin: 18px 0;
  }
  .name {
    font-size: 13.5px;
    font-weight: 560;
    margin-bottom: 10px;
  }
  .val {
    color: var(--text-3);
    font-weight: 400;
    margin-left: 6px;
  }
  .swatches {
    display: flex;
    flex-wrap: wrap;
    gap: 10px;
  }
  .sw {
    width: 28px;
    height: 28px;
    border-radius: 50%;
    display: grid;
    place-items: center;
    box-shadow: inset 0 0 0 1px rgba(255, 255, 255, 0.12);
    transition: transform 0.12s;
  }
  .sw:hover {
    transform: scale(1.12);
  }
  .sw.dark {
    box-shadow: inset 0 0 0 1px rgba(255, 255, 255, 0.25);
  }
  .modes {
    display: grid;
    grid-template-columns: repeat(4, minmax(0, 1fr));
    gap: 10px;
  }
  .mode {
    text-align: left;
    padding: 8px 8px 12px;
    border-radius: 14px;
    background: rgba(255, 255, 255, 0.04);
    border: 1px solid var(--line);
    transition: border-color 0.15s, background 0.15s;
  }
  .mode:hover {
    background: rgba(255, 255, 255, 0.07);
  }
  .mode.sel {
    border-color: var(--accent);
  }
  .prev {
    height: 64px;
    border-radius: 9px;
    margin-bottom: 10px;
    background-size: cover;
    background-position: center;
  }
  .prev.glass {
    background: linear-gradient(135deg, rgba(255, 255, 255, 0.18), rgba(255, 255, 255, 0.04)), #2a3038;
  }
  .prev.kiosk {
    background-image: url("/kiosk-bg.webp");
  }
  .prev.image {
    background-color: rgba(255, 255, 255, 0.06);
  }
  .ml {
    font-weight: 580;
    font-size: 13.5px;
  }
  input[type="range"] {
    width: 100%;
    max-width: 360px;
    accent-color: var(--accent);
  }
  .toggle {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 20px;
    padding: 12px 0;
    border-top: 1px solid var(--line);
    cursor: pointer;
  }
  .toggle:first-of-type {
    border-top: 0;
  }
  .toggle input {
    appearance: none;
    width: 40px;
    height: 23px;
    flex: none;
    border-radius: 999px;
    background: rgba(255, 255, 255, 0.16);
    position: relative;
    cursor: pointer;
    transition: background 0.2s;
  }
  .toggle input::after {
    content: "";
    position: absolute;
    top: 3px;
    left: 3px;
    width: 17px;
    height: 17px;
    border-radius: 50%;
    background: #fff;
    transition: transform 0.2s;
  }
  .toggle input:checked {
    background: var(--accent);
  }
  .toggle input:checked::after {
    transform: translateX(17px);
    background: #1a1a1a;
  }
  .form {
    display: grid;
    grid-template-columns: 2fr 1fr;
    gap: 12px;
    margin-top: 6px;
    transition: opacity 0.2s;
  }
  .form.off {
    opacity: 0.45;
    pointer-events: none;
  }
  .form label {
    display: flex;
    flex-direction: column;
    gap: 6px;
    font-size: 12.5px;
    color: var(--text-2);
  }
  .form .wide {
    grid-column: 1 / -1;
  }
  .save {
    display: flex;
    justify-content: flex-end;
    margin-top: 14px;
  }
  .status {
    font-size: 12.5px;
    padding: 5px 10px;
    border-radius: 999px;
    background: rgba(230, 120, 120, 0.15);
    color: #f38ba8;
    white-space: nowrap;
  }
  .status.ok {
    background: rgba(130, 200, 150, 0.15);
    color: #a6e3a1;
  }
</style>
