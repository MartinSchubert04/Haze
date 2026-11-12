<script lang="ts">
  // Confirmar antes de borrar, con el estilo de Haze en vez del cartel de Windows. Suena el
  // aviso de Ethereon (assets/sounds/approval.wav) al aparecer.
  import { TriangleAlert } from "@lucide/svelte";
  import { store } from "../store.svelte";
  import approval from "../../assets/sounds/approval.wav?url";

  let ok = $state<HTMLButtonElement>();

  $effect(() => {
    if (!store.confirmDialog) return;
    const sound = new Audio(approval);
    sound.volume = 0.6;
    sound.play().catch(() => {});
    queueMicrotask(() => ok?.focus());
    return () => sound.pause();
  });

  function answer(yes: boolean) {
    const c = store.confirmDialog;
    if (!c) return;
    store.confirmDialog = null;
    c.resolve(yes);
  }
</script>

<svelte:window onkeydown={(e) => store.confirmDialog && e.key === "Escape" && answer(false)} />

{#if store.confirmDialog}
  <!-- svelte-ignore a11y_no_static_element_interactions, a11y_click_events_have_key_events -->
  <div class="backdrop" onclick={() => answer(false)}></div>
  <div class="dialog" role="alertdialog" aria-modal="true" aria-labelledby="confirm-title" aria-describedby="confirm-text">
    <div class="icon"><TriangleAlert size={20} /></div>
    <div class="body">
      <h3 id="confirm-title">{store.confirmDialog.title}</h3>
      <p id="confirm-text">{store.confirmDialog.text}</p>
      <div class="actions">
        <button class="pill" onclick={() => answer(false)}>Cancelar</button>
        <button class="pill danger" bind:this={ok} onclick={() => answer(true)}>{store.confirmDialog.action}</button>
      </div>
    </div>
  </div>
{/if}

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    z-index: 95;
    background: rgba(0, 0, 0, 0.35);
  }
  .dialog {
    position: fixed;
    z-index: 96;
    left: 50%;
    top: 30%;
    transform: translateX(-50%);
    width: min(420px, 90vw);
    display: flex;
    gap: 14px;
    padding: 20px;
    border-radius: var(--radius-lg);
    background: var(--pop);
    backdrop-filter: var(--blur);
    border: var(--border) solid var(--line-strong);
    box-shadow: var(--pop-shadow);
    animation: pop 0.14s ease-out;
  }
  @keyframes pop {
    from {
      opacity: 0;
      transform: translateX(-50%) scale(0.97);
    }
  }
  .icon {
    flex: none;
    width: 36px;
    height: 36px;
    display: grid;
    place-items: center;
    border-radius: 50%;
    color: var(--danger);
    background: color-mix(in srgb, var(--danger) 14%, transparent);
  }
  .body {
    flex: 1;
    min-width: 0;
  }
  h3 {
    margin: 6px 0 6px;
    font-size: 16px;
    overflow-wrap: anywhere;
  }
  p {
    margin: 0;
    color: var(--text-2);
    font-size: 13.5px;
    line-height: 1.45;
  }
  .actions {
    display: flex;
    justify-content: flex-end;
    gap: 10px;
    margin-top: 18px;
  }
  .danger {
    background: var(--danger);
    border-color: transparent;
    color: #fff;
  }
  .danger:hover {
    background: color-mix(in srgb, var(--danger) 85%, #fff);
  }
</style>
