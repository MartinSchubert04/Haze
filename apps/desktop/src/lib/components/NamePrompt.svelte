<script lang="ts">
  // Pide un nombre: lista nueva o renombrar.
  import { store } from "../store.svelte";

  let value = $state("");
  let input = $state<HTMLInputElement>();

  $effect(() => {
    const p = store.namePrompt;
    if (!p) return;
    value = p.value;
    queueMicrotask(() => {
      input?.focus();
      input?.select();
    });
  });

  function close() {
    store.namePrompt = null;
  }

  function save() {
    const p = store.namePrompt;
    const name = value.trim();
    if (!p || !name) return;
    close();
    p.onSave(name);
  }
</script>

{#if store.namePrompt}
  <!-- svelte-ignore a11y_no_static_element_interactions, a11y_click_events_have_key_events -->
  <div class="backdrop" onclick={close}></div>
  <form
    class="dialog"
    onsubmit={(e) => {
      e.preventDefault();
      save();
    }}
  >
    <h3>{store.namePrompt.title}</h3>
    <input bind:this={input} type="text" bind:value maxlength="80" placeholder="Nombre" onkeydown={(e) => e.key === "Escape" && close()} />
    <div class="actions">
      <button type="button" class="pill" onclick={close}>Cancelar</button>
      <button type="submit" class="pill primary" disabled={!value.trim()}>{store.namePrompt.action}</button>
    </div>
  </form>
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
    width: min(380px, 90vw);
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
  h3 {
    margin: 0 0 14px;
    font-size: 16px;
  }
  input {
    width: 100%;
  }
  .actions {
    display: flex;
    justify-content: flex-end;
    gap: 10px;
    margin-top: 16px;
  }
</style>
