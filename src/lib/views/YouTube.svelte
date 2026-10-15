<script lang="ts">
  import { Search, Link2, LoaderCircle, TvMinimalPlay } from "@lucide/svelte";
  import { api, isUrl, type Song } from "../api";
  import { store } from "../store.svelte";
  import SongList from "../components/SongList.svelte";

  let { query = "" }: { query?: string } = $props();

  let text = $state("");
  let results = $state.raw<Song[]>([]);
  let searched = $state("");
  let loading = $state(false);
  let error = $state<string | null>(null);

  // Arranca con lo que se escribio arriba (Enter en la busqueda general).
  $effect(() => {
    if (query && query !== searched) {
      text = query;
      run();
    }
  });

  async function run(e?: Event) {
    e?.preventDefault();
    const t = text.trim();
    if (!t) return;
    if (isUrl(t)) {
      store.pendingLink = t;
      return;
    }
    loading = true;
    error = null;
    searched = t;
    try {
      results = await api.ytSearch(t);
    } catch (err) {
      error = String(err);
      results = [];
    } finally {
      loading = false;
    }
  }
</script>

<SongList songs={results} context={`YouTube: ${searched}`} showFolder={false} onDownload={(s) => (store.pendingLink = s.path)}>
  {#snippet header()}
    <div class="head">
      <h1 class="page-title">YouTube</h1>
      <div class="page-sub">Buscá para escuchar sin bajar, o pegá un link para bajarlo como MP3.</div>
      <form class="bar" onsubmit={run}>
        {#if isUrl(text)}<Link2 size={18} />{:else}<Search size={18} />{/if}
        <input type="text" placeholder="Buscar en YouTube o pegar un link (video o playlist)" bind:value={text} />
        <button class="pill primary" type="submit" disabled={loading || !text.trim()}>
          {#if loading}<LoaderCircle size={16} class="spin" />{/if}
          {isUrl(text) ? "Abrir link" : "Buscar"}
        </button>
      </form>
      {#if error}<div class="error">{error}</div>{/if}
      {#if searched && !loading && !error}
        <div class="page-sub results">{results.length} resultados para "{searched}"</div>
      {/if}
    </div>
  {/snippet}
</SongList>

{#if !searched && !loading}
  <div class="hint">
    <TvMinimalPlay size={40} strokeWidth={1.4} />
    <p>Tip: en cualquier parte de la app podés pegar un link con <kbd>Ctrl V</kbd> y te pregunta si lo querés bajar.</p>
  </div>
{/if}

<style>
  .head {
    padding: 8px 0 12px;
  }
  .bar {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 6px 6px 6px 16px;
    border-radius: 999px;
    background: rgba(255, 255, 255, 0.06);
    border: 1px solid var(--line);
    color: var(--text-3);
    max-width: 760px;
  }
  .bar:focus-within {
    border-color: var(--line-strong);
    background: rgba(255, 255, 255, 0.09);
  }
  .bar input {
    flex: 1;
    background: none;
    border: 0;
    padding: 6px 0;
    font-size: 14.5px;
    color: var(--text);
  }
  .error {
    margin-top: 12px;
    color: #f38ba8;
    font-size: 13px;
  }
  .results {
    margin: 16px 0 4px;
  }
  .hint {
    position: absolute;
    left: 0;
    right: 0;
    top: 46%;
    display: grid;
    justify-items: center;
    gap: 6px;
    color: var(--text-3);
    text-align: center;
    pointer-events: none;
    padding: 0 40px;
  }
  kbd {
    font: inherit;
    font-size: 12px;
    border: 1px solid var(--line-strong);
    border-radius: 6px;
    padding: 1px 6px;
  }
</style>
