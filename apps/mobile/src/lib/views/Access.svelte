<script lang="ts">
  // Sin permiso para leer la musica no hay biblioteca: se explica y se pide.
  import { Music } from "@lucide/svelte";
  import { api } from "../api";
  import { store } from "../store.svelte";

  let asking = $state(false);
  let denied = $state(false);

  async function ask() {
    asking = true;
    const granted = await api.requestAccess();
    asking = false;
    store.access = granted;
    denied = !granted;
  }
</script>

<div class="access">
  <div class="icon"><Music size={40} strokeWidth={1.6} /></div>
  <h2>Your music</h2>
  <p>Haze needs permission to see the music stored on your phone. It doesn't upload or share anything.</p>
  <button class="pill primary" disabled={asking} onclick={ask}>Allow</button>
  {#if denied}
    <p class="hint">
      If Android doesn't show the prompt anymore, turn it on by hand: phone Settings › Apps › Haze › Permissions › Music and
      audio.
    </p>
  {/if}
</div>

<style>
  .access {
    height: 100%;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 14px;
    padding: 32px;
    text-align: center;
  }
  .icon {
    width: 88px;
    height: 88px;
    display: grid;
    place-items: center;
    border-radius: 50%;
    background: rgb(var(--ov) / 0.06);
    color: var(--accent);
  }
  h2 {
    margin: 8px 0 0;
    font-size: 24px;
  }
  p {
    margin: 0;
    color: var(--text-2);
    line-height: 1.5;
  }
  .pill {
    height: 46px;
    padding: 0 28px;
    margin-top: 8px;
  }
  .hint {
    font-size: 13.5px;
  }
</style>
