<script lang="ts">
  import { tip } from "./lib/shortcuts";
  // Ventanita fija: tapa, titulo y controles. Se arrastra desde cualquier parte.
  // Reemplaza a la ventana grande: al abrirse la cierra, y al expandirla se cierra ella.
  import { onMount } from "svelte";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { SkipBack, SkipForward, X, Maximize2, Shuffle, Repeat, Repeat1 } from "@lucide/svelte";
  import { api } from "./lib/api";
  import { store } from "./lib/store.svelte";
  import Background from "./lib/components/Background.svelte";
  import Cover from "./lib/components/Cover.svelte";
  import PlayButton from "./lib/components/PlayButton.svelte";

  const win = getCurrentWindow();
  const p = $derived(store.player);
  const frac = $derived(p.duration > 0 ? Math.min(1, store.position / p.duration) : 0);
  const nextRepeat = $derived(p.repeat === "off" ? "all" : p.repeat === "all" ? "one" : "off");
  const repeatLabel = $derived(p.repeat === "off" ? "Repetir: no" : p.repeat === "all" ? "Repetir: todo" : "Repetir: una");

  // Si el texto no entra, al pasar el mouse por la ventana se desliza hasta el final y
  // vuelve, para que se lea completo.
  function marquee(node: HTMLElement) {
    const measure = () => {
      const extra = node.scrollWidth - node.clientWidth;
      node.style.setProperty("--shift", `${-Math.max(0, extra)}px`);
      node.style.setProperty("--dur", `${Math.max(3, extra / 25 + 2)}s`);
      node.classList.toggle("long", extra > 0);
    };
    const root = node.closest(".mini");
    root?.addEventListener("mouseenter", measure);
    return { destroy: () => root?.removeEventListener("mouseenter", measure) };
  }

  onMount(async () => {
    await store.init();
    await win.show();
    api.windowReady("mini");
  });
</script>

<svelte:window onkeydown={(e) => e.code === "Space" && (e.preventDefault(), api.control({ type: "toggle" }))} />

<Background />

<div class="mini" data-tauri-drag-region>
  <div class="cov" data-tauri-drag-region>
    <Cover song={p.song} size={64} radius={10} />
  </div>

  <div class="side" data-tauri-drag-region>
    <div class="meta" data-tauri-drag-region>
      <div class="t scroll" use:marquee data-tauri-drag-region><span data-tauri-drag-region>{p.song?.title ?? "Nada sonando"}</span></div>
      <div class="a scroll" use:marquee data-tauri-drag-region><span data-tauri-drag-region>{p.song?.artist ?? ""}</span></div>
    </div>

    <div class="controls">
      <button class="icon-btn sm" class:on={p.shuffle} title={p.shuffle ? "Aleatorio: sí" : "Aleatorio: no (orden normal)"} onclick={() => api.control({ type: "shuffle", value: !p.shuffle })}>
        <Shuffle size={14} />
      </button>
      <button class="icon-btn" title={tip("Anterior", "previous")} onclick={() => api.control({ type: "previous" })}><SkipBack size={16} fill="currentColor" /></button>
      <PlayButton size={36} ring={false} solid />
      <button class="icon-btn" title={tip("Siguiente", "next")} onclick={() => api.control({ type: "next" })}><SkipForward size={16} fill="currentColor" /></button>
      <button class="icon-btn sm" class:on={p.repeat !== "off"} title={repeatLabel} onclick={() => api.control({ type: "repeat", value: nextRepeat })}>
        {#if p.repeat === "one"}<Repeat1 size={14} />{:else}<Repeat size={14} />{/if}
      </button>
    </div>
  </div>

  <div class="win">
    <button class="icon-btn s" title={tip("Abrir Haze", "mini")} onclick={() => api.showMain()}><Maximize2 size={13} /></button>
    <button class="icon-btn s" title="Cerrar (sigue sonando)" onclick={() => win.close()}><X size={15} /></button>
  </div>
  <div class="progress"><div class="fill" style:width="{frac * 100}%"></div></div>
</div>

<style>
  .mini {
    position: relative;
    height: 100vh;
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 0 10px;
    overflow: hidden;
  }
  .cov {
    flex: none;
  }
  .side {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .meta {
    text-align: center;
  }
  /* Los botones de expandir y cerrar estan arriba a la derecha: el titulo no pasa por ahi. */
  .t {
    margin-right: 50px;
    max-width: 98px;
  }
  .scroll {
    overflow: hidden;
    white-space: nowrap;
    text-overflow: ellipsis;
  }
  .scroll span {
    display: inline-block;
  }
  .mini:hover .scroll:global(.long) {
    text-overflow: clip;
    text-align: left;
  }
  .mini:hover .scroll:global(.long) span {
    animation: marquee var(--dur) ease-in-out 0.4s infinite alternate;
  }
  @keyframes marquee {
    0%,
    15% {
      transform: translateX(0);
    }
    85%,
    100% {
      transform: translateX(var(--shift));
    }
  }
  .t {
    font-weight: 600;
    font-size: 13.5px;
    justify-self: left;
  }
  .a {
    color: var(--text-2);
    font-size: 12px;
    margin-top: 1px;
    min-height: 15px;
    justify-self: left;
  }
  .controls {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 2px;
  }
  .controls .icon-btn {
    width: 30px;
    height: 30px;
  }
  .controls .sm {
    width: 26px;
    height: 26px;
  }
  .win {
    position: absolute;
    top: 6px;
    right: 6px;
    display: flex;
    gap: 2px;
    opacity: 0;
    transition: opacity 0.15s;
  }
  .mini:hover .win {
    opacity: 1;
  }
  .s {
    width: 24px;
    height: 24px;
  }
  .progress {
    position: absolute;
    left: 0;
    right: 0;
    bottom: 0;
    height: 3px;
    background: rgb(var(--ov) / 0.08);
  }
  .fill {
    height: 100%;
    background: var(--accent);
    transition: width 0.25s linear;
  }
</style>
