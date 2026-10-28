<script lang="ts">
  // Lo que suena, encima de las pestañas. Tocarlo abre "Reproduciendo".
  import { SkipBack, SkipForward } from "@lucide/svelte";
  import { api } from "../api";
  import { store } from "../store.svelte";
  import Cover from "./Cover.svelte";
  import PlayButton from "./PlayButton.svelte";

  const p = $derived(store.player);
  const frac = $derived(p.duration > 0 ? Math.min(1, store.position / p.duration) : 0);

  const open = () => store.open({ kind: "nowPlaying" });
</script>

<div class="mini press" role="button" tabindex="0" onclick={open} onkeydown={(e) => e.key === "Enter" && open()}>
  <div class="progress" style:width="{frac * 100}%"></div>
  <Cover song={p.song} size={44} radius={8} />
  <div class="text">
    <div class="title ellipsis">{p.song?.title}</div>
    <div class="sub ellipsis">{p.song?.artist || p.context}</div>
  </div>
  <button
    class="icon-btn"
    aria-label="Anterior"
    onclick={(e) => {
      e.stopPropagation();
      api.control({ type: "previous" });
    }}
  >
    <SkipBack size={22} fill="currentColor" />
  </button>
  <PlayButton size={44} />
  <button
    class="icon-btn"
    aria-label="Siguiente"
    onclick={(e) => {
      e.stopPropagation();
      api.control({ type: "next" });
    }}
  >
    <SkipForward size={22} fill="currentColor" />
  </button>
</div>

<style>
  .mini {
    position: relative;
    height: var(--mini);
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 0 8px 0 12px;
    border-bottom: 1px solid var(--line);
  }
  .progress {
    position: absolute;
    left: 0;
    top: -1px;
    height: 2px;
    background: var(--accent);
    transition: width 0.25s linear;
  }
  .text {
    flex: 1;
    min-width: 0;
    margin-left: 4px;
  }
  .title {
    font-weight: 600;
  }
  .sub {
    font-size: 13px;
    color: var(--text-2);
  }
  .icon-btn {
    color: var(--text);
  }
</style>
