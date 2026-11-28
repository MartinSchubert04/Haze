<script lang="ts">
  // Ajustes del telefono, en secciones que se pliegan: apariencia, reproduccion,
  // sincronizar, Home Assistant, biblioteca y actualizaciones.
  import { onMount } from "svelte";
  import {
    ChevronLeft,
    Check,
    RefreshCw,
    Download,
    Eye,
    EyeOff,
    Undo2,
    Palette,
    Play,
    House,
    Library,
    CircleArrowUp,
  } from "@lucide/svelte";
  import { THEMES, palettesOf, paletteOf, themeOf } from "@haze/ui/themes";
  import { folderName, plural, songCount } from "../format";
  import { api } from "../api";
  import { store } from "../store.svelte";
  import Section from "../components/Section.svelte";
  import Select from "../components/Select.svelte";

  const BACKGROUNDS = ["#0f1720", "#000000", "#121212", "#1a1423", "#102019", "#1b1412", "#1d2328"];

  const s = $derived(store.settings!);
  const a = $derived(s.appearance);
  const theme = $derived(themeOf(a.theme));
  const palette = $derived(paletteOf(theme, a.palette));
  const themeName = $derived(THEMES.find((t) => t.id === theme)?.name ?? "");
  const paletteName = $derived(palettesOf(theme).find((p) => p.id === palette)?.name ?? "");

  // Abierta desde el menu del engranaje con una seccion: esa arranca desplegada.
  onMount(() => {
    const layer = [...store.layers].reverse().find((l) => l.kind === "settings");
    if (layer?.kind === "settings" && layer.section) store.settingsOpen[layer.section] = true;
  });

  // Home Assistant se edita en un borrador y se guarda con un boton: si no, cada letra
  // reconectaria al broker.
  let ha = $state({ ...store.settings!.ha });
  const haDirty = $derived(JSON.stringify(ha) !== JSON.stringify(s.ha));
  const haLabel = $derived(
    ({ connected: "Connected", error: "Can't connect", network: "Needs permission" } as Record<string, string>)[store.haStatus] ??
      "Connecting…",
  );

  // Server y usuario se guardan con un boton, como HA: cada letra arrancaria una vuelta.
  let syncDraft = $state({ server: store.settings!.sync.server, user: store.settings!.sync.user });
  const syncDirty = $derived(
    syncDraft.server.trim() !== s.sync.server || (syncDraft.user.trim() || "default") !== s.sync.user,
  );
  const syncing = $derived(store.syncReport.status === "running");

  // Cada cuanto: un numero y la unidad. Se guarda en minutos; al abrir, si es un numero
  // justo de horas se muestra en horas.
  const UNITS = [
    { id: "min" as const, label: "Minutes", short: "min" },
    { id: "h" as const, label: "Hours", short: "h" },
  ];
  const startMin = store.settings!.sync.interval || 5;
  const inHours = startMin % 60 === 0;
  let unit = $state<"min" | "h">(inHours ? "h" : "min");
  let amount = $state(inHours ? startMin / 60 : startMin);
  const everyText = $derived(
    s.sync.interval % 60 === 0 ? `${s.sync.interval / 60} h` : `${s.sync.interval} min`,
  );

  function saveInterval() {
    const n = Math.max(1, Math.round(Number(amount) || 0));
    amount = n;
    const minutes = Math.min(n * (unit === "h" ? 60 : 1), 7 * 24 * 60);
    if (minutes !== s.sync.interval) setSync({ interval: minutes });
  }

  function setSync(patch: Partial<typeof s.sync>) {
    return store.saveSettings((x) => (x.sync = { ...x.sync, ...patch }));
  }

  // El permiso de red local se pide antes de prender: asi la primera vuelta ya lo tiene.
  async function toggleSync(on: boolean) {
    if (on) await api.requestLocalNetwork().catch(() => false);
    await setSync({ enabled: on });
  }

  async function grantNetwork() {
    if (await api.requestLocalNetwork().catch(() => false)) api.syncNow();
  }

  const ago = (ms: number) => {
    const m = Math.round((Date.now() - ms) / 60000);
    if (m < 1) return "just now";
    if (m < 60) return `${m} min ago`;
    const h = Math.round(m / 60);
    return h < 24 ? `${h} h ago` : `${Math.round(h / 24)} d ago`;
  };

  const syncLine = $derived.by(() => {
    const r = store.syncReport;
    const p = store.syncProgress;
    if (!s.sync.enabled) return "Off.";
    if (!s.sync.server.trim()) return "Set the server first.";
    if (r.status === "running") return p && p.total ? `Syncing ${p.done} of ${p.total}…` : "Syncing…";
    if (r.status === "network") return "Needs the local network permission.";
    if (r.status === "away") return `Away from home: couldn't find the server (${ago(r.at)}).`;
    if (r.status === "error") return `Error ${ago(r.at)}: ${r.error ?? "something went wrong"}`;
    if (r.status === "ok") {
      const parts = [r.downloaded && `${r.downloaded} downloaded`, r.uploaded && `${r.uploaded} uploaded`].filter(Boolean);
      return `Up to date · ${ago(r.at)}${parts.length ? ` · ${parts.join(", ")}` : ""}`;
    }
    return "Hasn't synced yet.";
  });

  const syncSummary = $derived(
    !s.sync.enabled
      ? "Off"
      : syncing
        ? "Syncing…"
        : store.syncReport.status === "ok"
          ? `Up to date · ${ago(store.syncReport.at)}`
          : store.syncReport.status === "away"
            ? "Away from home"
            : store.syncReport.status === "error"
              ? "Error"
              : "On",
  );

  // El numero cambia mientras se arrastra; se guarda (y se aplica al audio) al soltar.
  let boost = $state(store.settings!.playback.boost ?? 0);
  const boostLabel = (db: number) => (db > 0 ? `+${db} dB` : "Off");
  const playbackSummary = $derived(
    [s.playback.resume && "Resume on", s.playback.boost > 0 && `Boost ${boostLabel(s.playback.boost)}`]
      .filter(Boolean)
      .join(" · "),
  );

  const removedList = $derived(store.userdata.removed.filter((r) => r.removed).sort((a, b) => b.updated - a.updated));

  async function saveHa() {
    await store.saveSettings((x) => (x.ha = { ...ha, port: Number(ha.port) || 1883 }));
    // Despues de guardar: si dan el permiso, Rust reconecta con lo nuevo.
    if (ha.enabled) await api.requestLocalNetwork();
  }
</script>

<section class="layer">
  <div class="page">
    <button class="icon-btn back" aria-label="Back" onclick={() => store.close()}><ChevronLeft size={26} /></button>
    <h1 class="page-title">Settings</h1>

    <Section id="appearance" title="Appearance" icon={Palette} summary="{themeName} · {paletteName}">
      <h3>Theme</h3>
      <div class="themes">
        {#each THEMES as t (t.id)}
          {@const sw = palettesOf(t.id)[0]?.swatch ?? []}
          <button
            class="theme press"
            class:on={theme === t.id}
            onclick={() => store.saveSettings((x) => ((x.appearance.theme = t.id), (x.appearance.palette = "")))}
          >
            <span class="tprev" style:background={sw[0]}>
              <span class="tpanel" style:background={sw[1]}></span>
              <span class="tdot" style:background={sw[2]}></span>
            </span>
            <span class="ellipsis">{t.name}</span>
          </button>
        {/each}
      </div>

      <h3>Palette</h3>
      <div class="palettes">
        {#each palettesOf(theme) as p (p.id)}
          <button
            class="palette press"
            class:on={palette === p.id}
            onclick={() => store.saveSettings((x) => ((x.appearance.theme = theme), (x.appearance.palette = p.id)))}
          >
            <span class="psw">{#each p.swatch as c}<span style:background={c}></span>{/each}</span>
            {p.name}
          </button>
        {/each}
      </div>

      <h3>Background</h3>
      <div class="choices">
        <button class="choice press" class:on={a.background !== "color"} onclick={() => store.saveSettings((x) => (x.appearance.background = "theme"))}>
          <span class="preview theme"></span> From theme
        </button>
        <button class="choice press" class:on={a.background === "color"} onclick={() => store.saveSettings((x) => (x.appearance.background = "color"))}>
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
      {/if}

      <label class="row">
        <span>Tint "Now playing" with the cover</span>
        <input
          type="checkbox"
          checked={a.tintFromArtwork}
          onchange={(e) => {
            const v = e.currentTarget.checked;
            store.saveSettings((x) => (x.appearance.tintFromArtwork = v));
          }}
        />
      </label>
    </Section>

    <Section id="playback" title="Playback" icon={Play} summary={playbackSummary}>
      <label class="row">
        <span>Resume the last song when opening</span>
        <input
          type="checkbox"
          checked={s.playback.resume}
          onchange={(e) => {
            const v = e.currentTarget.checked;
            store.saveSettings((x) => (x.playback.resume = v));
          }}
        />
      </label>
      <label class="slider">
        <span class="row-top"><span>Volume boost</span><span class="value">{boostLabel(boost)}</span></span>
        <input
          type="range"
          min="0"
          max="12"
          step="1"
          bind:value={boost}
          style:--fill="{(boost / 12) * 100}%"
          onchange={() => store.saveSettings((x) => (x.playback.boost = boost))}
        />
        <span class="hint">For headphones that sound quiet even with the phone at full volume. Too much can distort.</span>
      </label>
    </Section>

    <Section id="sync" title="Sync" icon={RefreshCw} summary={syncSummary} ok={s.sync.enabled && store.syncReport.status === "ok"}>
      <p class="hint">
        At home, when you open Haze and every {everyText} while it's open, it downloads to Music/Haze whatever the phone
        is missing, uploads anything new to the server and merges your lists. It never deletes files. Away from home it does
        nothing.
      </p>
      <label class="row">
        <span>Sync with the home server</span>
        <input type="checkbox" checked={s.sync.enabled} onchange={(e) => toggleSync(e.currentTarget.checked)} />
      </label>
      <div class="form" class:off={!s.sync.enabled}>
        <label>Server<input type="text" inputmode="url" bind:value={syncDraft.server} placeholder="192.168.0.100" autocapitalize="off" /></label>
        <label>
          User<input type="text" bind:value={syncDraft.user} placeholder="default" autocomplete="off" autocapitalize="off" />
          <span class="hint">Use the same one on the PC and the phone to share lists.</span>
        </label>
        <button
          class="pill primary save"
          disabled={!syncDirty}
          onclick={() => setSync({ server: syncDraft.server.trim(), user: syncDraft.user.trim() || "default" })}
        >
          Save
        </button>
      </div>
      <div class="row" class:off={!s.sync.enabled}>
        <span>Sync every</span>
        <div class="every">
          <input
            type="number"
            inputmode="numeric"
            min="1"
            bind:value={amount}
            onchange={saveInterval}
            onkeydown={(e) => e.key === "Enter" && e.currentTarget.blur()}
          />
          <Select
            label="Unit"
            icon={null}
            value={unit}
            options={UNITS}
            onchange={(u) => ((unit = u), saveInterval())}
          />
        </div>
      </div>
      <div class="row">
        <span class="hint sync-line" class:err={store.syncReport.status === "error" && s.sync.enabled}>{syncLine}</span>
        {#if s.sync.enabled && store.syncReport.status === "network" && !syncing}
          <button class="pill" onclick={grantNetwork}>Allow</button>
        {:else}
          <button class="pill" disabled={!s.sync.enabled || syncing} onclick={() => api.syncNow()}>
            <RefreshCw size={15} class={syncing ? "spin" : ""} /> Sync now
          </button>
        {/if}
      </div>
    </Section>

    <Section
      id="ha"
      title="Home Assistant"
      icon={House}
      summary={s.ha.enabled ? haLabel : "Off"}
      ok={s.ha.enabled && store.haStatus === "connected"}
    >
      <p class="hint">Over MQTT. The entities show up in HA on their own, under the "{s.ha.deviceName}" device.</p>
      <label class="row">
        <span>Connect to Home Assistant</span>
        <input type="checkbox" bind:checked={ha.enabled} />
      </label>
      <div class="form" class:off={!ha.enabled}>
        <label>MQTT server<input type="text" inputmode="url" bind:value={ha.host} placeholder="192.168.1.10" /></label>
        <label>Port<input type="number" inputmode="numeric" bind:value={ha.port} /></label>
        <label>User<input type="text" bind:value={ha.username} autocomplete="off" autocapitalize="off" /></label>
        <label>Password<input type="password" bind:value={ha.password} autocomplete="off" /></label>
        <label>
          Device name<input type="text" bind:value={ha.deviceName} />
          <span class="hint">If you use Haze on more than one device, give each one a different name.</span>
        </label>
        {#if s.ha.enabled && store.haStatus === "network"}
          <span class="hint">Android asks for permission to let Haze reach the local network (it calls it "nearby devices").</span>
          <button class="pill save" onclick={() => api.requestLocalNetwork()}>Allow</button>
        {/if}
        <button class="pill primary save" disabled={!haDirty} onclick={saveHa}>Save</button>
      </div>
    </Section>

    <Section id="library" title="Library" icon={Library} summary={songCount(store.uniqueSongs.length)}>
      <div class="row">
        <span>{songCount(store.uniqueSongs.length)} on the phone</span>
        <button class="pill" disabled={store.scanning} onclick={() => api.rescan()}>
          <RefreshCw size={15} class={store.scanning ? "spin" : ""} /> Rescan
        </button>
      </div>
      {#if s.hiddenFolders?.length}
        <h3>Hidden folders</h3>
        {#each s.hiddenFolders as path (path)}
          <div class="row hidden-folder">
            <div class="folder-text">
              <div class="ellipsis">{folderName(path) || "Storage"}</div>
              <div class="hint ellipsis">{path || "/"}</div>
            </div>
            <button class="pill" onclick={() => store.showFolder(path)}><Eye size={15} /> Show</button>
          </div>
        {/each}
      {/if}
      {#if removedList.length}
        <h3>Removed songs</h3>
        <p class="hint">Hidden on every device, but the file is still there.</p>
        {#each removedList as r (r.file)}
          <div class="row hidden-folder">
            <div class="folder-text removed">
              <EyeOff size={16} />
              <span class="ellipsis">{r.title || "Untitled"}</span>
            </div>
            <button class="pill" onclick={() => store.restore(r.file)}><Undo2 size={15} /> Restore</button>
          </div>
        {/each}
      {/if}
    </Section>

    <Section
      id="updates"
      title="Updates"
      icon={CircleArrowUp}
      summary={store.update?.status === "ready" ? `${store.update.version} ready` : `Version ${store.version}`}
      ok={store.update?.status === "ready"}
    >
      <div class="row">
        <span>Version {store.version}</span>
        {#if store.update?.status === "ready"}
          <button class="pill primary" onclick={() => store.installUpdate()}>Update now</button>
        {:else}
          <button class="pill" onclick={() => store.checkUpdate()} disabled={store.checkingUpdate || !!store.update}>
            <RefreshCw size={15} class={store.checkingUpdate || store.update ? "spin" : ""} /> Check
          </button>
        {/if}
      </div>
      {#if store.update}
        <div class="update">
          <Download size={18} />
          <span>
            {store.update.status === "ready"
              ? `Haze ${store.update.version} is ready. It installs itself when you leave the app with nothing playing.`
              : `Downloading Haze ${store.update.version}… ${Math.round(store.update.progress * 100)}%`}
          </span>
        </div>
      {/if}
    </Section>
  </div>
  <div class="spacer-bottom"></div>
</section>

<style>
  .layer {
    position: fixed;
    inset: 0;
    z-index: 10;
    overflow-y: auto;
    background: var(--bg);
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
  .page-title {
    margin-bottom: 14px;
  }
  h3 {
    font-size: 12.5px;
    font-weight: 600;
    letter-spacing: 0.04em;
    text-transform: uppercase;
    color: var(--text-3);
    margin: 18px 0 10px;
  }
  h3:first-child {
    margin-top: 8px;
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
  .removed {
    display: flex;
    align-items: center;
    gap: 10px;
    color: var(--text-2);
  }
  .sync-line {
    margin: 0;
    min-width: 0;
  }
  .sync-line.err {
    color: var(--danger);
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
    border: var(--border) solid var(--line-strong);
    transition: transform 0.4s cubic-bezier(0.2, 0.8, 0.2, 1) 0.06s;
  }
  .swatch:global(.pressed) {
    transform: scale(0.88);
    transition-duration: 0.08s;
    transition-delay: 0s;
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
    border-radius: calc(14px * var(--rs));
    border: var(--border) solid var(--line);
  }
  .choice.on {
    border-color: var(--accent);
  }
  .preview {
    width: 32px;
    height: 32px;
    border-radius: calc(8px * var(--rs));
  }
  .preview.theme {
    background-color: var(--bg);
    background-image: var(--bg-image);
    border: var(--border) solid var(--line);
  }
  .themes {
    display: grid;
    grid-template-columns: repeat(3, minmax(0, 1fr));
    gap: 10px;
  }
  .theme {
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 8px 8px 10px;
    border-radius: calc(14px * var(--rs));
    border: var(--border) solid var(--line);
    background: rgb(var(--ov) / 0.04);
    font-size: 13.5px;
    text-align: left;
  }
  .theme.on {
    border-color: var(--accent);
  }
  .tprev {
    position: relative;
    display: block;
    height: 46px;
    border-radius: calc(9px * var(--rs));
    overflow: hidden;
    box-shadow: inset 0 0 0 1px rgb(var(--ov) / 0.1);
  }
  .tpanel {
    position: absolute;
    inset: 10px 10px 0 24px;
    border-radius: calc(6px * var(--rs)) calc(6px * var(--rs)) 0 0;
  }
  .tdot {
    position: absolute;
    left: 8px;
    bottom: 8px;
    width: 11px;
    height: 11px;
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
    height: 38px;
    padding: 0 14px 0 9px;
    border-radius: var(--round);
    border: var(--border) solid var(--line);
    background: rgb(var(--ov) / 0.04);
    font-size: 14px;
  }
  .palette.on {
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
    height: 20px;
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
    flex: none;
    accent-color: var(--accent);
  }
  .slider {
    display: flex;
    flex-direction: column;
    gap: 10px;
    padding: 12px 0 6px;
  }
  .row-top {
    display: flex;
    justify-content: space-between;
  }
  .value {
    color: var(--text-2);
    font-variant-numeric: tabular-nums;
  }
  input[type="range"] {
    appearance: none;
    width: 100%;
    height: 28px;
    margin: 0;
    background: transparent;
  }
  input[type="range"]::-webkit-slider-runnable-track {
    height: 4px;
    border-radius: 2px;
    background: linear-gradient(to right, var(--accent) var(--fill), rgb(var(--ov) / 0.15) var(--fill));
  }
  input[type="range"]::-webkit-slider-thumb {
    appearance: none;
    width: 20px;
    height: 20px;
    margin-top: -8px;
    border-radius: 50%;
    background: var(--accent);
  }
  .hint {
    font-size: 13px;
    color: var(--text-3);
    margin: 0 0 4px;
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
    border-radius: calc(12px * var(--rs));
    border: var(--border) solid var(--line);
    background: rgb(var(--ov) / 0.05);
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
  .row.off {
    opacity: 0.45;
    pointer-events: none;
  }
  .every {
    display: flex;
    align-items: center;
    gap: 10px;
  }
  .every input {
    width: 64px;
    height: 34px;
    padding: 0 10px;
    border-radius: calc(10px * var(--rs));
    border: var(--border) solid var(--line-strong);
    background: rgb(var(--ov) / 0.04);
    color: var(--text);
    font: inherit;
    font-size: 15px;
    text-align: center;
  }
  .every input:focus {
    outline: none;
    border-color: var(--accent);
  }
  .update {
    display: flex;
    align-items: center;
    gap: 10px;
    font-size: 13.5px;
    color: var(--text-2);
  }
</style>
