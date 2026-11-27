<script lang="ts">
  // Una seccion de Ajustes que se pliega: el titulo con un resumen a la derecha y, al
  // tocarlo, el contenido. Queda abierta o cerrada como se dejo (store.settingsOpen).
  import { onMount, type Component, type Snippet } from "svelte";
  import { slide } from "svelte/transition";
  import { cubicOut } from "svelte/easing";
  import { ChevronDown } from "@lucide/svelte";
  import { store } from "../store.svelte";

  let {
    id,
    title,
    icon: Icon,
    summary = "",
    ok = false,
    children,
  }: {
    id: string;
    title: string;
    icon: Component<{ size?: number }>;
    /** Lo que se ve plegado, al lado del titulo (el tema, "On", el estado de HA). */
    summary?: string;
    /** El resumen va en verde (conectado, al dia). */
    ok?: boolean;
    children: Snippet;
  } = $props();

  const open = $derived(!!store.settingsOpen[id]);
  let root: HTMLElement;

  function toggle() {
    store.settingsOpen[id] = !open;
  }

  // Abierta desde el menu del engranaje (Ajustes con `section`): se lleva a la vista.
  onMount(() => {
    if (store.layers.some((l) => l.kind === "settings" && l.section === id)) {
      setTimeout(() => root?.scrollIntoView({ behavior: "smooth", block: "start" }), 220);
    }
  });
</script>

<section class="section" class:open bind:this={root}>
  <button class="head fold" aria-expanded={open} onclick={toggle}>
    <span class="icon"><Icon size={20} /></span>
    <span class="title">{title}</span>
    {#if summary}<span class="summary ellipsis" class:ok>{summary}</span>{/if}
    <span class="chev"><ChevronDown size={18} /></span>
  </button>
  {#if open}
    <div class="body" transition:slide={{ duration: 220, easing: cubicOut }}>
      <div class="inner">{@render children()}</div>
    </div>
  {/if}
</section>

<style>
  .section {
    margin-top: 10px;
    border-radius: calc(16px * var(--rs));
    border: var(--border) solid var(--line);
    background: rgb(var(--ov) / 0.03);
    transition:
      border-color 0.2s,
      background 0.2s;
  }
  .section.open {
    border-color: var(--line-strong);
    background: rgb(var(--ov) / 0.05);
  }
  .head {
    width: 100%;
    display: flex;
    align-items: center;
    gap: 12px;
    min-height: 56px;
    padding: 0 14px;
    text-align: left;
    color: var(--text);
    transition: opacity var(--release);
  }
  .head:global(.pressed) {
    opacity: 0.6;
    transition: opacity 0.05s;
  }
  .icon {
    display: grid;
    place-items: center;
    width: 34px;
    height: 34px;
    flex: none;
    border-radius: calc(10px * var(--rs));
    background: color-mix(in srgb, var(--accent) 14%, transparent);
    color: var(--accent);
  }
  .title {
    flex: none;
    font-size: 15.5px;
    font-weight: 600;
  }
  .summary {
    flex: 1;
    min-width: 0;
    text-align: right;
    font-size: 13px;
    color: var(--text-3);
  }
  .summary.ok {
    color: var(--ok);
  }
  .chev {
    display: grid;
    margin-left: auto;
    color: var(--text-3);
    transition: transform 0.22s cubic-bezier(0.2, 0.8, 0.2, 1);
  }
  .summary + .chev {
    margin-left: 0;
  }
  .open .chev {
    transform: rotate(180deg);
  }
  .inner {
    padding: 2px 14px 14px;
  }
</style>
