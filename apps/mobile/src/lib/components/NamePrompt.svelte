<script lang="ts">
  // El nombre de una lista: crear (con las canciones que traiga) o renombrar. Va arriba y
  // no abajo como las hojas: abajo lo tapaba el teclado.
  import { onMount, tick } from "svelte";
  import type { Song } from "../api";
  import { store } from "../store.svelte";

  let { id, songs }: { id?: string; songs?: Song[] } = $props();

  const list = $derived(id ? store.list(id) : undefined);
  let name = $state("");
  let input: HTMLInputElement;

  // Al renombrar, el nombre de ahora queda marcado para escribir encima.
  onMount(async () => {
    name = (id && store.list(id)?.name) || "";
    await tick();
    input.focus();
    input.select();
  });

  // Se lee todo antes de cerrar: despues la capa ya no existe.
  async function save() {
    const n = name.trim();
    if (!n) return;
    const listId = id;
    const add = songs ?? [];
    store.close();
    if (listId) await store.editUser({ type: "rename", id: listId, name: n });
    else await store.createList(n, add);
  }
</script>

<div class="backdrop" role="presentation" onclick={() => store.close()}></div>
<form
  class="dialog"
  aria-label={list ? "Renombrar lista" : "Nueva lista"}
  onsubmit={(e) => {
    e.preventDefault();
    save();
  }}
>
  <h3>{list ? "Renombrar lista" : "Nueva lista"}</h3>
  {#if !list && songs?.length === 1}<p class="hint ellipsis">Con {songs[0].title}</p>{/if}
  <input type="text" bind:this={input} bind:value={name} placeholder="Nombre" maxlength="80" enterkeyhint="done" />
  <div class="buttons">
    <button type="button" class="pill" onclick={() => store.close()}>Cancelar</button>
    <button type="submit" class="pill primary" disabled={!name.trim()}>{list ? "Guardar" : "Crear"}</button>
  </div>
</form>

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    z-index: 60;
    background: rgba(0, 0, 0, 0.5);
    animation: fade 0.2s ease-out;
  }
  .dialog {
    position: fixed;
    left: 16px;
    right: 16px;
    top: calc(var(--safe-top) + 72px);
    z-index: 61;
    display: flex;
    flex-direction: column;
    gap: 12px;
    padding: 18px 16px 16px;
    border-radius: calc(20px * var(--rs));
    /* --pop puede ser translucido: debajo va el fondo del tema. */
    background-color: var(--bg);
    background-image: linear-gradient(var(--pop), var(--pop));
    border: 1px solid var(--line-strong);
    box-shadow: var(--pop-shadow);
    animation: down 0.22s cubic-bezier(0.2, 0.8, 0.2, 1);
  }
  h3 {
    margin: 0;
    font-size: 17px;
  }
  .hint {
    margin: -6px 0 0;
    font-size: 13px;
    color: var(--text-3);
  }
  input {
    height: 46px;
    padding: 0 14px;
    border-radius: calc(12px * var(--rs));
    border: var(--border) solid var(--line);
    background: rgb(var(--ov) / 0.05);
    color: var(--text);
    font: inherit;
    font-size: 16px;
  }
  input:focus {
    outline: none;
    border-color: var(--accent);
  }
  .buttons {
    display: flex;
    justify-content: flex-end;
    gap: 10px;
  }
  @keyframes down {
    from {
      opacity: 0;
      transform: translateY(-12px);
    }
  }
  @keyframes fade {
    from {
      opacity: 0;
    }
  }
</style>
