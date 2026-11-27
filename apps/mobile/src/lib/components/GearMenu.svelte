<script lang="ts">
  // El engranaje de Canciones: al tocarlo gira y despliega un menu chico con Ajustes,
  // Sincronizar y Actualizar. Sync y Update actuan de una y avisan con un toast.
  import { Settings as SettingsIcon, SlidersHorizontal, RefreshCw, Download, CircleArrowUp } from "@lucide/svelte";
  import { ripple } from "../ripple";
  import { store } from "../store.svelte";

  let open = $state(false);
  let root: HTMLDivElement;

  const syncing = $derived(store.syncReport.status === "running");
  const syncOn = $derived(!!store.settings?.sync.enabled && !!store.settings.sync.server.trim());
  const updateLine = $derived(
    store.update?.status === "ready"
      ? `${store.update.version} ready · tap to install`
      : store.update
        ? `Downloading ${store.update.version}… ${Math.round(store.update.progress * 100)}%`
        : `Version ${store.version}`,
  );

  // Se cierra tocando afuera o con "atras", como el desplegable de ordenar.
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

  function run(action: () => unknown) {
    open = false;
    action();
  }
</script>

<div class="gear" class:open bind:this={root}>
  <button class="icon-btn" aria-label="Menu" aria-haspopup="menu" aria-expanded={open} onclick={() => (open = !open)}>
    <span class="cog"><SettingsIcon size={22} /></span>
  </button>

  {#if open}
    <div class="menu" role="menu">
      <button class="item" role="menuitem" use:ripple onclick={() => run(() => store.open({ kind: "settings" }))}>
        <span class="ripple-host"></span>
        <SlidersHorizontal size={20} />
        <span class="text"><span class="name">Settings</span><span class="sub">Theme, Home Assistant and more</span></span>
      </button>
      <button class="item" role="menuitem" use:ripple onclick={() => run(() => store.syncNow())}>
        <span class="ripple-host"></span>
        <RefreshCw size={20} class={syncing ? "spin" : ""} />
        <span class="text">
          <span class="name">Sync</span>
          <span class="sub">{syncing ? "Syncing…" : syncOn ? "With the home server" : "Off"}</span>
        </span>
      </button>
      <button class="item" role="menuitem" use:ripple onclick={() => run(() => store.checkUpdate())}>
        <span class="ripple-host"></span>
        {#if store.update?.status === "ready"}<CircleArrowUp size={20} class="ready" />
        {:else}<Download size={20} class={store.checkingUpdate ? "bounce" : ""} />{/if}
        <span class="text"><span class="name">Update</span><span class="sub">{updateLine}</span></span>
      </button>
    </div>
  {/if}
</div>

<style>
  .gear {
    position: relative;
    z-index: 20;
  }
  .cog {
    display: grid;
    transition: transform 0.45s cubic-bezier(0.2, 0.8, 0.2, 1);
  }
  .open .cog {
    transform: rotate(75deg);
    color: var(--accent);
  }
  .menu {
    position: absolute;
    top: calc(100% + 4px);
    right: 0;
    width: 260px;
    padding: 6px;
    border-radius: calc(16px * var(--rs));
    background: var(--pop);
    border: var(--border) solid var(--line-strong);
    box-shadow: var(--pop-shadow);
    transform-origin: top right;
    animation: grow 0.2s cubic-bezier(0.2, 0.8, 0.2, 1);
  }
  @keyframes grow {
    from {
      opacity: 0;
      transform: scale(0.7, 0.55);
    }
  }
  .item {
    position: relative;
    width: 100%;
    display: flex;
    align-items: center;
    gap: 14px;
    padding: 10px 12px;
    border-radius: calc(11px * var(--rs));
    text-align: left;
    color: var(--text);
  }
  .item > :global(svg) {
    flex: none;
    color: var(--text-2);
  }
  .item > :global(svg.ready) {
    color: var(--accent);
  }
  .item > :global(svg.bounce) {
    animation: bounce 0.7s ease-in-out infinite;
  }
  @keyframes bounce {
    50% {
      transform: translateY(3px);
    }
  }
  .text {
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 1px;
  }
  .name {
    font-size: 15px;
    font-weight: 600;
  }
  .sub {
    font-size: 12.5px;
    color: var(--text-3);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .ripple-host {
    position: absolute;
    inset: 0;
    border-radius: inherit;
    overflow: hidden;
    pointer-events: none;
  }
</style>
