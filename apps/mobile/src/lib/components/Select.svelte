<script lang="ts" generics="T extends string">
  // Desplegable al estilo del Select de MUI (campo con borde y label flotante, menu que
  // crece desde el campo), con los colores de Haze.
  import type { Component } from "svelte";
  import { ChevronDown, Check } from "@lucide/svelte";
  import { ripple } from "../ripple";

  type Option = { id: T; label: string; icon?: Component<{ size?: number }> };

  let {
    label,
    value,
    options,
    onchange,
  }: { label: string; value: T; options: Option[]; onchange: (id: T) => void } = $props();

  let open = $state(false);
  let root: HTMLDivElement;
  const id = `select-${Math.random().toString(36).slice(2, 8)}`;
  const current = $derived(options.find((o) => o.id === value) ?? options[0]);

  function pick(o: Option) {
    open = false;
    if (o.id !== value) onchange(o.id);
  }

  // Se cierra tocando afuera o con Escape.
  $effect(() => {
    if (!open) return;
    const outside = (e: PointerEvent) => {
      if (!root.contains(e.target as Node)) open = false;
    };
    const key = (e: KeyboardEvent) => {
      if (e.key === "Escape") open = false;
    };
    window.addEventListener("pointerdown", outside, true);
    window.addEventListener("keydown", key);
    return () => {
      window.removeEventListener("pointerdown", outside, true);
      window.removeEventListener("keydown", key);
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
    {#if current.icon}<current.icon size={16} />{/if}
    <span class="value" id="{id}-value">{current.label}</span>
    <ChevronDown size={18} class="chev" />
  </button>

  {#if open}
    <ul class="menu" role="listbox" aria-labelledby="{id}-label">
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
  /* Ancho fijo: el campo y el menu miden lo mismo y no saltan al cambiar de opcion. */
  .select {
    position: relative;
    z-index: 5;
    width: 172px;
  }
  .field {
    position: relative;
    width: 100%;
    display: flex;
    align-items: center;
    gap: 8px;
    height: 40px;
    padding: 0 10px 0 14px;
    border-radius: 12px;
    background: rgba(255, 255, 255, 0.04);
    color: var(--text);
    font-size: 14px;
    font-weight: 550;
    white-space: nowrap;
    transition: background 0.2s;
  }
  .open .field {
    background: rgba(255, 255, 255, 0.07);
  }
  .outline {
    position: absolute;
    inset: -6px 0 0;
    margin: 0;
    padding: 0 8px;
    border: 1px solid var(--line-strong);
    border-radius: 12px;
    text-align: left;
    pointer-events: none;
    transition: border-color 0.2s;
  }
  .open .outline {
    border-color: var(--accent);
  }
  .outline legend {
    height: 11px;
    padding: 0;
    font-size: 11px;
    font-weight: 600;
    letter-spacing: 0.02em;
    white-space: nowrap;
    visibility: hidden;
  }
  .outline legend span {
    display: inline-block;
    padding: 0 4px;
  }
  /* El label se apoya sobre el borde, en el hueco que abre el legend. */
  .label {
    position: absolute;
    top: 0;
    left: 13px;
    transform: translateY(-50%);
    font-size: 11px;
    font-weight: 600;
    letter-spacing: 0.02em;
    line-height: 1;
    color: var(--text-3);
    transition: color 0.2s;
  }
  .open .label {
    color: var(--accent);
  }
  .value {
    flex: 1;
    text-align: left;
  }
  .field :global(.chev) {
    color: var(--text-2);
    transition: transform 0.22s cubic-bezier(0.2, 0.8, 0.2, 1);
  }
  .open .field :global(.chev) {
    transform: rotate(180deg);
    color: var(--accent);
  }

  .menu {
    position: absolute;
    top: calc(100% + 6px);
    left: 0;
    right: 0;
    margin: 0;
    padding: 6px;
    list-style: none;
    border-radius: 14px;
    background: #1d2328;
    border: 1px solid var(--line-strong);
    box-shadow:
      0 5px 5px -3px rgba(0, 0, 0, 0.3),
      0 8px 10px 1px rgba(0, 0, 0, 0.22),
      0 3px 14px 2px rgba(0, 0, 0, 0.2);
    transform-origin: top center;
    animation: grow 0.18s cubic-bezier(0.2, 0.8, 0.2, 1);
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
    height: 46px;
    padding: 0 10px 0 12px;
    border-radius: 10px;
    font-size: 15px;
    color: var(--text);
    text-align: left;
  }
  .item span {
    flex: 1;
  }
  .item :global(svg) {
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
