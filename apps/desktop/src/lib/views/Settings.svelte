<script lang="ts">
  import { shortcutList, defaultShortcuts, prettyShortcut, shortcutFromEvent, type ShortcutKey } from "../shortcuts";
  import { open } from "@tauri-apps/plugin-dialog";
  import { openUrl } from "@tauri-apps/plugin-opener";
  import { enable, disable, isEnabled } from "@tauri-apps/plugin-autostart";
  import { FolderPlus, Trash2, RefreshCw, Star, ImagePlus, Check, Download, ChevronDown, RotateCcw } from "@lucide/svelte";
  import { api, fileUrl } from "../api";
  import { store } from "../store.svelte";
  import { folderName, plural } from "@haze/ui/format";
  import { THEMES, palettesOf, paletteOf, themeOf } from "@haze/ui/themes";

  const ACCENTS = [
    "#ffffff", "#f5e0dc", "#f2cdcd", "#f5c2e7", "#cba6f7", "#f38ba8", "#eba0ac", "#fab387",
    "#f9e2af", "#a6e3a1", "#94e2d5", "#89dceb", "#74c7ec", "#89b4fa", "#b4befe",
  ];
  const BACKGROUNDS = ["#0f1720", "#000000", "#121212", "#1a1423", "#102019", "#1b1412", "#1d2328"];
  const MODES: [string, string, string][] = [
    ["theme", "Tema", "El fondo del tema y su paleta"],
    ["glass", "Vidrio", "Desenfoca lo que hay detrás de la ventana"],
    ["image", "Imagen", "Una imagen tuya"],
    ["color", "Color", "Un color liso"],
  ];

  const s = $derived(store.settings!);
  const theme = $derived(themeOf(s.appearance.theme));
  const palette = $derived(paletteOf(theme, s.appearance.palette));
  // "haze" es la bruma de antes de los temas.
  const bgMode = $derived(s.appearance.background === "haze" ? "theme" : s.appearance.background);
  let autostart = $state(false);
  isEnabled().then((v) => (autostart = v), () => {});

  // Home Assistant se edita en un borrador y se guarda con un boton: si no, cada letra
  // que se tipea reconectaria MQTT.
  let ha = $state({ ...store.settings!.ha });
  const haDirty = $derived(JSON.stringify(ha) !== JSON.stringify(s.ha));

  // El token tambien va en borrador: cada letra reconectaria el bot.
  let botToken = $state(store.settings!.discord.token);
  const tokenDirty = $derived(botToken.trim() !== s.discord.token);
  const botLabel = $derived(
    ({ online: store.discord.channel ? "En un canal" : "Conectado", connecting: "Conectando…", error: "Error" } as Record<string, string>)[
      store.discord.state
    ] ?? "Apagado",
  );

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

  function toggleFav(dir: string) {
    store.saveSettings((x) => {
      if (x.downloadFolders.includes(dir)) {
        x.downloadFolders = x.downloadFolders.filter((p) => p !== dir);
        if (x.defaultDownloadFolder === dir) x.defaultDownloadFolder = x.downloadFolders[0] ?? null;
      } else {
        x.downloadFolders = [...x.downloadFolders, dir];
        x.defaultDownloadFolder ??= dir;
      }
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

  let checking = $state(false);
  let upToDate = $state(false);

  async function checkUpdate() {
    checking = true;
    upToDate = false;
    try {
      upToDate = !(await api.checkUpdate());
    } catch (e) {
      store.toast(`No se pudo buscar actualizaciones: ${e}`, "error");
    } finally {
      checking = false;
    }
  }

  // Atajo que se esta eligiendo: la proxima combinacion que se apriete lo reemplaza.
  let editing = $state<ShortcutKey | null>(null);

  function editShortcut(key: ShortcutKey) {
    editing = key;
    api.pauseShortcuts(true);
  }

  function stopEditing() {
    if (!editing) return;
    editing = null;
    api.pauseShortcuts(false);
  }

  function setShortcut(key: ShortcutKey, acc: string) {
    store.saveSettings((x) => {
      // Una combinacion es de un solo atajo: si ya la tenia otro, se la saca.
      for (const [k] of shortcutList) if (k !== key && x.shortcuts[k] === acc) x.shortcuts[k] = "";
      x.shortcuts[key] = acc;
    });
  }

  function onKey(e: KeyboardEvent) {
    if (!editing) return;
    // Captura: que no le llegue a los atajos de la ventana (espacio = play, etc.).
    e.preventDefault();
    e.stopImmediatePropagation();
    if (e.code === "Escape") return stopEditing();
    if (e.code === "Backspace" || e.code === "Delete") {
      setShortcut(editing, "");
      return stopEditing();
    }
    const acc = shortcutFromEvent(e);
    if (!acc) return;
    setShortcut(editing, acc);
    stopEditing();
  }

  $effect(() => stopEditing);

  const count = (dir: string) => store.songs.filter((x) => x.path.toLowerCase().startsWith(dir.toLowerCase() + "/")).length;
</script>

<svelte:window onkeydowncapture={onKey} />

<div class="scroll">
  <div class="page">
    <h1 class="page-title">Ajustes</h1>

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
          {@const fav = s.downloadFolders.includes(src)}
          <div class="item">
            <button class="icon-btn" class:on={fav} title={fav ? "Quitar de favoritas" : "Agregar a favoritas"} onclick={() => toggleFav(src)}>
              <Star size={16} fill={fav ? "currentColor" : "none"} />
            </button>
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
        <div class="name">Tema</div>
        <div class="themes">
          {#each THEMES as t}
            {@const sw = palettesOf(t.id)[0]?.swatch ?? []}
            <button class="theme" class:sel={theme === t.id} onclick={() => store.saveSettings((x) => ((x.appearance.theme = t.id), (x.appearance.palette = "")))}>
              <div class="tprev" style:background={sw[0]}>
                <div class="tpanel" style:background={sw[1]}></div>
                <div class="tdot" style:background={sw[2]}></div>
              </div>
              <div class="ml">{t.name}</div>
            </button>
          {/each}
        </div>
      </div>

      <div class="field">
        <div class="name">Paleta</div>
        <div class="palettes">
          {#each palettesOf(theme) as p}
            <button class="palette" class:sel={palette === p.id} onclick={() => store.saveSettings((x) => ((x.appearance.theme = theme), (x.appearance.palette = p.id)))}>
              <span class="psw">{#each p.swatch as c}<span style:background={c}></span>{/each}</span>
              {p.name}
            </button>
          {/each}
        </div>
      </div>

      <div class="field">
        <div class="name">Fondo</div>
        <div class="modes">
          {#each MODES as [k, label, hint]}
            <button class="mode" class:sel={bgMode === k} onclick={() => (k === "image" && !s.appearance.backgroundImage ? pickImage() : store.saveSettings((x) => (x.appearance.background = k as never)))}>
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

      {#if bgMode === "image" || bgMode === "glass"}
        <div class="field">
          <div class="name">Oscurecer <span class="val">{Math.round(s.appearance.dim * 100)}%</span></div>
          <input type="range" min="0" max="0.9" step="0.01" value={s.appearance.dim} onchange={(e) => store.saveSettings((x) => (x.appearance.dim = +e.currentTarget.value))} oninput={(e) => store.settings && (store.settings.appearance.dim = +e.currentTarget.value)} />
        </div>
      {/if}
      {#if bgMode === "image" || bgMode === "theme"}
        <div class="field">
          <div class="name">Desenfoque <span class="val">{Math.round(s.appearance.blur)} px</span></div>
          <input type="range" min="0" max="40" step="1" value={s.appearance.blur} onchange={(e) => store.saveSettings((x) => (x.appearance.blur = +e.currentTarget.value))} oninput={(e) => store.settings && (store.settings.appearance.blur = +e.currentTarget.value)} />
        </div>
      {/if}

      <div class="field">
        <div class="name">Color de las carpetas</div>
        <div class="swatches">
          <button class="sw accent" title="El acento de la paleta" onclick={() => store.saveSettings((x) => (x.appearance.folderColor = ""))}>
            {#if !s.appearance.folderColor}<Check size={14} />{/if}
          </button>
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

    <!-- ------------------------------------------------------------ atajos -->
    <section>
      <h2>Atajos de teclado</h2>
      <p>Andan desde cualquier programa. Tocá uno para cambiarlo; Esc cancela y Borrar lo deja sin atajo.</p>
      <div class="keys">
        {#each shortcutList as [key, label]}
          {@const acc = s.shortcuts[key]}
          <div class="key-row">
            <span class="grow">{label}</span>
            {#if acc !== defaultShortcuts[key] && editing !== key}
              <button class="icon-btn sm" title="Volver al original" onclick={() => setShortcut(key, defaultShortcuts[key])}>
                <RotateCcw size={14} />
              </button>
            {/if}
            <button class="kbd" class:rec={editing === key} class:none={!acc} onclick={() => (editing === key ? stopEditing() : editShortcut(key))} onblur={stopEditing}>
              {editing === key ? "Apretá la combinación…" : prettyShortcut(acc)}
            </button>
          </div>
        {/each}
      </div>
    </section>

    <!-- ------------------------------------------------------------ home assistant -->
    <!-- Desplegable: la mayoria lo configura una vez y no lo vuelve a tocar. -->
    <details class="fold">
      <summary>
        <div class="grow">
          <h2>Home Assistant</h2>
          <p>Por MQTT. Las entidades aparecen solas en HA, en el dispositivo "{s.ha.deviceName}".</p>
        </div>
        {#if s.ha.enabled}
          <span class="status" class:ok={store.haStatus === "connected"}>
            {store.haStatus === "connected" ? "Conectado" : store.haStatus === "error" ? "Sin conexión" : "Conectando…"}
          </span>
        {/if}
        <ChevronDown size={18} class="chev" />
      </summary>
      <label class="toggle">
        <div>Conectar con Home Assistant</div>
        <input type="checkbox" bind:checked={ha.enabled} />
      </label>
      <div class="form" class:off={!ha.enabled}>
        <label>Servidor MQTT<input type="text" bind:value={ha.host} placeholder="192.168.1.10" /></label>
        <label>Puerto<input type="number" bind:value={ha.port} /></label>
        <label>Usuario<input type="text" bind:value={ha.username} autocomplete="off" /></label>
        <label>Contraseña<input type="password" bind:value={ha.password} autocomplete="off" /></label>
        <label class="wide">
          Nombre del dispositivo<input type="text" bind:value={ha.deviceName} />
          <span class="hint">Si usás Haze en más de una PC, poné un nombre distinto en cada una.</span>
        </label>
      </div>
      <div class="save">
        <button class="pill primary" disabled={!haDirty} onclick={() => store.saveSettings((x) => (x.ha = { ...ha, port: Number(ha.port) || 1883 }))}>
          Guardar
        </button>
      </div>
    </details>

    <!-- ------------------------------------------------------------ discord -->
    <details class="fold">
      <summary>
        <div class="grow">
          <h2>Discord</h2>
          <p>Un bot que entra a tu canal de voz y pone lo que suena en Haze. Los demás del canal suman temas con /play.</p>
        </div>
        {#if s.discord.enabled}
          <span class="status" class:ok={store.discord.state === "online"}>{botLabel}</span>
        {/if}
        <ChevronDown size={18} class="chev" />
      </summary>
      <label class="toggle">
        <div>
          <div>Prender el bot</div>
          <div class="hint">Apagado no usa nada. Prendido, espera a que entres a un canal de voz de un servidor donde esté el bot.</div>
        </div>
        <input
          type="checkbox"
          checked={s.discord.enabled}
          onchange={(e) => {
            const on = e.currentTarget.checked;
            store.saveSettings((x) => (x.discord.enabled = on));
          }}
        />
      </label>
      <label class="toggle">
        <div>
          <div>Silenciar la PC mientras suena en Discord</div>
          <div class="hint">Lo escuchás por Discord, igual que los demás, sin eco.</div>
        </div>
        <input
          type="checkbox"
          checked={s.discord.muteLocal}
          onchange={(e) => {
            const on = e.currentTarget.checked;
            store.saveSettings((x) => (x.discord.muteLocal = on));
          }}
        />
      </label>
      <div class="form">
        <label class="wide">
          Token del bot<input type="password" bind:value={botToken} autocomplete="off" placeholder="Pegalo desde el portal de Discord" />
          <span class="hint">
            discord.com/developers → New Application → Bot → Reset Token. El bot sigue al dueño de la aplicación (vos).
          </span>
        </label>
      </div>
      {#if store.discord.state === "error" || store.discord.error}
        <p class="bot-error">{store.discord.error}</p>
      {/if}
      {#if store.discord.state === "online"}
        <p class="bot-info">
          Conectado como <b>{store.discord.bot}</b>{#if store.discord.channel} · sonando en {store.discord.channel}{/if}
        </p>
      {/if}
      <div class="save">
        {#if store.discord.invite}
          <button class="pill" onclick={() => openUrl(store.discord.invite!)}>Sumar a un servidor</button>
        {/if}
        <button class="pill primary" disabled={!tokenDirty} onclick={() => store.saveSettings((x) => (x.discord.token = botToken.trim()))}>
          Guardar
        </button>
      </div>
    </details>

    <!-- ------------------------------------------------------------ actualizaciones -->
    <section>
      <div class="sec-head">
        <div>
          <h2>Actualizaciones</h2>
          <p>Versión {store.version}</p>
        </div>
        {#if store.update?.status === "ready"}
          <button class="pill primary" onclick={() => api.updateNow()}>Actualizar ahora</button>
        {:else}
          <button class="pill" onclick={checkUpdate} disabled={checking || !!store.update}>
            <RefreshCw size={15} class={checking || store.update ? "spin" : ""} /> {checking ? "Buscando…" : "Buscar actualizaciones"}
          </button>
        {/if}
      </div>
      {#if store.update}
        <div class="update">
          <Download size={18} />
          <div class="grow">
            {store.update.status === "ready" ? `Haze ${store.update.version} está lista` : `Bajando Haze ${store.update.version}…`}
          </div>
        </div>
      {:else if upToDate}
        <div class="update ok"><Check size={18} /> Estás en la última versión</div>
      {/if}
    </section>

    <section>
      <h2>Salir</h2>
      <p>Cierra la app del todo, incluida la música de fondo.</p>
      <button class="pill" onclick={() => api.quit()}>Salir de Haze</button>
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
    margin: 0 auto;
  }
  .fold {
    margin-top: 18px;
    padding: 0 22px;
    border-radius: var(--radius-lg);
    background: var(--tile);
  }
  .fold summary {
    display: flex;
    align-items: center;
    gap: 16px;
    padding: 20px 0;
    cursor: pointer;
    list-style: none;
  }
  .fold summary::-webkit-details-marker {
    display: none;
  }
  .fold summary p {
    margin: 0;
    color: var(--text-2);
    font-size: 13px;
  }
  .fold :global(.chev) {
    flex: none;
    color: var(--text-3);
    transition: transform 0.2s;
  }
  .fold[open] :global(.chev) {
    transform: rotate(180deg);
  }
  .bot-error {
    margin: 12px 0 0;
    font-size: 13px;
    color: var(--danger);
  }
  .bot-info {
    margin: 12px 0 0;
    font-size: 13px;
    color: var(--text-2);
  }
  .fold[open] {
    padding-bottom: 22px;
  }
  .keys {
    display: flex;
    flex-direction: column;
  }
  .key-row {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 9px 0;
    border-top: 1px solid var(--line);
    font-size: 13.5px;
  }
  .key-row:first-child {
    border-top: 0;
  }
  .key-row {
    gap: 8px;
  }
  .kbd {
    font-size: 12px;
    color: var(--text-2);
    padding: 3px 8px;
    border-radius: calc(6px * var(--rs));
    border: var(--border) solid var(--line-strong);
    background: rgb(var(--ov) / 0.04);
    transition: background 0.15s, border-color 0.15s;
  }
  .kbd:hover {
    background: rgb(var(--ov) / 0.09);
    color: var(--text);
  }
  .kbd.none {
    color: var(--text-3);
    border-style: dashed;
  }
  .kbd.rec {
    color: var(--accent);
    border-color: var(--accent);
    background: color-mix(in srgb, var(--accent) 12%, transparent);
  }
  .icon-btn.sm {
    color: var(--text-3);
  }
  .update {
    display: flex;
    align-items: center;
    gap: 14px;
    width: 100%;
    text-align: left;
    padding: 12px 16px;
    border-radius: var(--radius);
    background: color-mix(in srgb, var(--accent) 16%, transparent);
    border: var(--border) solid color-mix(in srgb, var(--accent) 45%, transparent);
    color: var(--accent);
    font-weight: 580;
  }
  .update.ok {
    gap: 10px;
    background: color-mix(in srgb, var(--ok) 12%, transparent);
    border-color: color-mix(in srgb, var(--ok) 30%, transparent);
    color: var(--ok);
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
    background: rgb(var(--ov) / 0.04);
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
    background: rgb(var(--ov) / 0.04);
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
    box-shadow: inset 0 0 0 1px rgb(var(--ov) / 0.12);
    transition: transform 0.12s;
  }
  .sw:hover {
    transform: scale(1.12);
  }
  .sw.dark {
    box-shadow: inset 0 0 0 1px rgb(var(--ov) / 0.25);
  }
  .modes {
    display: grid;
    grid-template-columns: repeat(4, minmax(0, 1fr));
    gap: 10px;
  }
  .mode {
    text-align: left;
    padding: 8px 8px 12px;
    border-radius: calc(14px * var(--rs));
    background: rgb(var(--ov) / 0.04);
    border: var(--border) solid var(--line);
    transition: border-color 0.15s, background 0.15s;
  }
  .mode:hover {
    background: rgb(var(--ov) / 0.07);
  }
  .mode.sel {
    border-color: var(--accent);
  }
  .prev {
    height: 64px;
    border-radius: calc(9px * var(--rs));
    margin-bottom: 10px;
    background-size: cover;
    background-position: center;
  }
  .prev.glass {
    background: linear-gradient(135deg, rgb(var(--ov) / 0.18), rgb(var(--ov) / 0.04)), #2a3038;
  }
  .prev.theme {
    background-color: var(--bg);
    background-image: var(--bg-image);
  }
  .sw.accent {
    background: var(--accent);
    color: var(--accent-ink);
  }
  .themes {
    display: grid;
    grid-template-columns: repeat(3, minmax(0, 1fr));
    gap: 10px;
  }
  .theme {
    text-align: left;
    padding: 8px 8px 10px;
    border-radius: calc(14px * var(--rs));
    background: rgb(var(--ov) / 0.04);
    border: var(--border) solid var(--line);
    transition: border-color 0.15s, background 0.15s;
  }
  .theme:hover {
    background: rgb(var(--ov) / 0.07);
  }
  .theme.sel {
    border-color: var(--accent);
  }
  .tprev {
    position: relative;
    height: 54px;
    border-radius: calc(9px * var(--rs));
    margin-bottom: 8px;
    overflow: hidden;
    box-shadow: inset 0 0 0 1px rgb(var(--ov) / 0.1);
  }
  .tpanel {
    position: absolute;
    inset: 12px 12px 0 28px;
    border-radius: calc(6px * var(--rs)) calc(6px * var(--rs)) 0 0;
  }
  .tdot {
    position: absolute;
    left: 10px;
    bottom: 10px;
    width: 12px;
    height: 12px;
    border-radius: 50%;
  }
  .palettes {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
  }
  .palette {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    height: 34px;
    padding: 0 12px 0 8px;
    border-radius: var(--round);
    background: rgb(var(--ov) / 0.04);
    border: var(--border) solid var(--line);
    font-size: 13px;
  }
  .palette:hover {
    background: rgb(var(--ov) / 0.08);
  }
  .palette.sel {
    border-color: var(--accent);
    color: var(--accent);
  }
  .psw {
    display: inline-flex;
    border-radius: var(--round);
    overflow: hidden;
    box-shadow: 0 0 0 1px rgb(var(--ov) / 0.15);
  }
  .psw span {
    width: 9px;
    height: 18px;
  }
  .prev.image {
    background-color: rgb(var(--ov) / 0.06);
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
    border-radius: var(--round);
    background: rgb(var(--ov) / 0.16);
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
    background: var(--text);
    transition: transform 0.2s;
  }
  .toggle input:checked {
    background: var(--accent);
  }
  .toggle input:checked::after {
    transform: translateX(17px);
    background: var(--accent-ink);
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
    gap: 10px;
    margin-top: 14px;
  }
  .status {
    font-size: 12.5px;
    padding: 5px 10px;
    border-radius: var(--round);
    background: color-mix(in srgb, var(--danger) 15%, transparent);
    color: var(--danger);
    white-space: nowrap;
  }
  .status.ok {
    background: color-mix(in srgb, var(--ok) 15%, transparent);
    color: var(--ok);
  }
</style>
