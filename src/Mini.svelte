<script lang="ts">
  // Ventanita fija: tapa, titulo y controles. Se arrastra desde cualquier parte.
  import { onMount } from "svelte";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { SkipBack, SkipForward, X, Maximize2 } from "@lucide/svelte";
  import { api } from "./lib/api";
  import { store } from "./lib/store.svelte";
  import Background from "./lib/components/Background.svelte";
  import Cover from "./lib/components/Cover.svelte";
  import PlayButton from "./lib/components/PlayButton.svelte";

  const win = getCurrentWindow();
  const p = $derived(store.player);
  const frac = $derived(p.duration > 0 ? Math.min(1, store.position / p.duration) : 0);

  onMount(async () => {
    await store.init();
    await win.show();
    let t: ReturnType<typeof setTimeout>;
    win.onMoved(({ payload }) => {
      clearTimeout(t);
      t = setTimeout(() => api.miniMoved(payload.x, payload.y), 400);
    });
  });
</script>

<Background />

<div class="mini" data-tauri-drag-region>
  <div class="cov" data-tauri-drag-region>
    <Cover song={p.song} size={64} radius={10} />
  </div>
  <div class="info" data-tauri-drag-region>
    <div class="t ellipsis" data-tauri-drag-region>{p.song?.title ?? "Nada sonando"}</div>
    <div class="a ellipsis" data-tauri-drag-region>{p.song?.artist ?? ""}</div>
    <div class="controls">
      <button class="icon-btn" title="Anterior" onclick={() => api.control({ type: "previous" })}><SkipBack size={16} fill="currentColor" /></button>
      <PlayButton size={34} ring={false} solid />
      <button class="icon-btn" title="Siguiente" onclick={() => api.control({ type: "next" })}><SkipForward size={16} fill="currentColor" /></button>
    </div>
  </div>
  <div class="win">
    <button class="icon-btn s" title="Abrir Haze" onclick={() => api.showMain()}><Maximize2 size={13} /></button>
    <button class="icon-btn s" title="Cerrar" onclick={() => win.close()}><X size={15} /></button>
  </div>
  <div class="progress"><div class="fill" style:width="{frac * 100}%"></div></div>
</div>

<style>
  .mini {
    position: relative;
    height: 100vh;
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 0 10px 0 14px;
    overflow: hidden;
  }
  .cov {
    flex: none;
  }
  .info {
    flex: 1;
    min-width: 0;
  }
  .t {
    font-weight: 600;
    font-size: 13.5px;
    padding-right: 44px;
  }
  .a {
    color: var(--text-2);
    font-size: 12px;
    margin-top: 1px;
  }
  .controls {
    display: flex;
    align-items: center;
    gap: 6px;
    margin-top: 5px;
  }
  .controls .icon-btn {
    width: 30px;
    height: 30px;
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
    background: rgba(255, 255, 255, 0.08);
  }
  .fill {
    height: 100%;
    background: var(--accent);
    transition: width 0.25s linear;
  }
</style>
