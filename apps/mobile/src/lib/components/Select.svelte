<script lang="ts" generics="T extends string">
  // Desplegable al estilo del Select de MUI (campo con borde y label flotante, menu que
  // crece desde el campo), con los colores de Haze. El campo es chico: una flecha, el
  // nombre corto del orden y el chevron; el menu, mas ancho, dice cada opcion completa.
  import type { Component } from "svelte";
  import { ArrowDownUp, ChevronDown, Check } from "@lucide/svelte";
  import { ripple } from "../ripple";
  import { store } from "../store.svelte";

  /** `short`: lo que se ve en el campo (si no, `label`). */
  type Option = { id: T; label: string; short?: string; icon?: Component<{ size?: number }> };

  let {
    label,
    value,
    options,
    onchange,
    align = "right",
  }: {
    label: string;
    value: T;
    options: Option[];
    onchange: (id: T) => void;
    /** De que lado del campo se abre el menu (el del borde de la pantalla mas cercano). */
    align?: "left" | "right";
  } = $props();

  let open = $state(false);
  let root: HTMLDivElement;
  const id = `select-${Math.random().toString(36).slice(2, 8)}`;
  const current = $derived(options.find((o) => o.id === value) ?? options[0]);

  function pick(o: Option) {
    open = false;
    if (o.id !== value) onchange(o.id);
  }

  // Se cierra tocando afuera o con "atras".
  $effect(() => {
    if (!open) return;
    const off = store.onBack("popup", () => {
      open = false;
      return true;
    });
    const outside = (e: PointerEvent) => {
      if (!root.contains(e.target as Node)) open = false;
    };
    window.addEventListener("pointerdown", outside, true);
    return () => {
      off();
      window.removeEventListener("pointerdown", outside, true);
    };
  });
</script>

<div class="select" class:open bind:this={root}>
  <button
    class="field"
    use:ripple
    aria-haspopup="listbox"
    aria-expanded={open}
    aria-labelledby="{id}-label {id}-value"
    onclick={() => (open = !open)}
  >
    <span class="ripple-host"></span>
    <!-- El borde es un fieldset: el legend (invisible) le abre el hueco al label, como en MUI. -->
    <fieldset class="outline" aria-hidden="true"><legend><span>{label}</span></legend></fieldset>
    <span class="label" id="{id}-label">{label}</span>
    <ArrowDownUp size={15} class="arrow" />
    <!-- Todos los nombres apilados en la misma celda: el campo mide lo que el mas largo y
         no salta al cambiar de opcion. -->
    <span class="value" id="{id}-value">
      {#each options as o (o.id)}
        <span class:on={o.id === current.id} aria-hidden={o.id !== current.id}>{o.short ?? o.label}</span>
      {/each}
    </span>
    <ChevronDown size={16} class="chev" />
  </button>

  {#if open}
    <ul class="menu {align}" role="listbox" aria-labelledby="{id}-label">
      {#each options as o (o.id)}
        <li role="option" aria-selected={o.id === value}>
          <button class="item ripple" class:on={o.id === value} use:ripple onclick={() => pick(o)}>
            {#if o.icon}<o.icon size={18} />{/if}
            <span>{o.label}</span>
            {#if o.id === value}<Check size={18} class="tick" />{/if}
          </button>
        </li>
      {/each}
    </ul>
  {/if}
</div>

<style>
  .select {
    position: relative;
    z-index: 5;
    width: max-content;
  }
  .field {
    position: relative;
    display: flex;
    align-items: center;
    gap: 6px;
    height: 34px;
    padding: 0 8px 0 10px;
    border-radius: calc(10px * var(--rs));
    background: rgb(var(--ov) / 0.04);
    color: var(--text);
    font-size: 13px;
    font-weight: 550;
    white-space: nowrap;
    transition: background 0.2s;
  }
  .open .field {
    background: rgb(var(--ov) / 0.07);
  }
  .outline {
    position: absolute;
    inset: -5px 0 0;
    margin: 0;
    padding: 0 6px;
    border: var(--border) solid var(--line-strong);
    border-radius: calc(10px * var(--rs));
    text-align: left;
    pointer-events: none;
    transition: border-color 0.2s;
  }
  .open .outline {
    border-color: var(--accent);
  }
  .outline legend {
    height: 10px;
    padding: 0;
    font-size: 10px;
    font-weight: 600;
    letter-spacing: 0.02em;
    white-space: nowrap;
    visibility: hidden;
  }
  .outline legend span {
    display: inline-block;
    padding: 0 3px;
  }
  /* El label se apoya sobre el borde, en el hueco que abre el legend. */
  .label {
    position: absolute;
    top: 0;
    left: 9px;
    transform: translateY(-50%);
    font-size: 10px;
    font-weight: 600;
    letter-spacing: 0.02em;
    line-height: 1;
    color: var(--text-3);
    transition: color 0.2s;
  }
  .open .label {
    color: var(--accent);
  }
  .field :global(.arrow) {
    flex: none;
    color: var(--text-2);
    transition: color 0.2s;
  }
  .open .field :global(.arrow) {
    color: var(--accent);
  }
  .value {
    display: grid;
    text-align: left;
  }
  .value span {
    grid-area: 1 / 1;
    visibility: hidden;
  }
  .value span.on {
    visibility: visible;
  }
  .field :global(.chev) {
    flex: none;
    color: var(--text-3);
    transition: transform 0.22s cubic-bezier(0.2, 0.8, 0.2, 1);
  }
  .open .field :global(.chev) {
    transform: rotate(180deg);
    color: var(--accent);
  }

  .menu {
    position: absolute;
    top: calc(100% + 6px);
    min-width: 200px;
    margin: 0;
    padding: 6px;
    list-style: none;
    border-radius: calc(14px * var(--rs));
    background: var(--pop);
    border: var(--border) solid var(--line-strong);
    box-shadow: var(--pop-shadow);
    animation: grow 0.18s cubic-bezier(0.2, 0.8, 0.2, 1);
  }
  .menu.right {
    right: 0;
    transform-origin: top right;
  }
  .menu.left {
    left: 0;
    transform-origin: top left;
  }
  @keyframes grow {
    from {
      opacity: 0;
      transform: scale(0.75, 0.6);
    }
  }
  .item {
    position: relative;
    width: 100%;
    display: flex;
    align-items: center;
    gap: 12px;
    height: 42px;
    padding: 0 10px 0 12px;
    border-radius: calc(10px * var(--rs));
    font-size: 14.5px;
    color: var(--text);
    text-align: left;
    white-space: nowrap;
  }
  .item span {
    flex: 1;
  }
  .item :global(svg) {
    flex: none;
    color: var(--text-2);
  }
  .item.on {
    background: color-mix(in srgb, var(--accent) 14%, transparent);
    color: var(--accent);
    font-weight: 600;
  }
  .item.on :global(svg) {
    color: var(--accent);
  }

  .ripple {
    overflow: hidden;
  }
  .ripple-host {
    position: absolute;
    inset: 0;
    border-radius: inherit;
    overflow: hidden;
    pointer-events: none;
  }
  :global(.ripple-wave) {
    position: absolute;
    border-radius: 50%;
    background: currentColor;
    opacity: 0.14;
    transform: scale(0);
    animation: wave 0.45s cubic-bezier(0.2, 0.8, 0.2, 1) forwards;
    pointer-events: none;
  }
  :global(.ripple-wave.out) {
    opacity: 0;
    transition: opacity 0.4s;
  }
  @keyframes wave {
    to {
      transform: scale(1);
    }
  }
</style>
