<script lang="ts">
  // Buscar en YouTube para escuchar sin bajar, o pegar un link para bajarlo. Arriba van
  // las descargas en curso.
  import { Search, Link2, X, LoaderCircle, TvMinimalPlay, Check, CircleAlert, Download } from "@lucide/svelte";
  import { isUrl } from "@haze/ui/format";
  import { api, type Song } from "../api";
  import { store } from "../store.svelte";
  import SongList from "../components/SongList.svelte";

  // Se recuerda al cambiar de pestaña: volver a YouTube no pierde la busqueda.
  let text = $state(last.text);
  let results = $state.raw<Song[]>(last.results);
  let searched = $state(last.searched);
  let loading = $state(false);
  let error = $state<string | null>(null);

  $effect(() => {
    last.text = text;
    last.results = results;
    last.searched = searched;
  });

  // Con una busqueda, "atras" la borra antes de cambiar de pestaña.
  $effect(() => {
    if (!text && !searched) return;
    return store.onBack("page", () => {
      text = "";
      searched = "";
      results = [];
      error = null;
      (document.activeElement as HTMLElement | null)?.blur();
      return true;
    });
  });

  const link = $derived(isUrl(text));
  const active = $derived(store.downloads.filter((d) => d.status !== "done" || d.error));
  const done = $derived(store.downloads.filter((d) => d.status === "done").length);

  async function run(e?: Event) {
    e?.preventDefault();
    const t = text.trim();
    if (!t) return;
    (document.activeElement as HTMLElement | null)?.blur();
    if (isUrl(t)) {
      store.open({ kind: "link", url: t });
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

  function label(status: string, item: string | null) {
    const base = { queued: "En espera", downloading: "Bajando", converting: "Convirtiendo", error: "Falló" }[status] ?? "";
    return item ? `${base} · ${item}` : base;
  }
</script>

<script module lang="ts">
  const last = { text: "", results: [] as Song[], searched: "" };
</script>

{#snippet header()}
  <div class="page">
    <h1 class="page-title">YouTube</h1>
    <div class="page-sub">Buscá para escuchar sin bajar, o pegá un link para bajarlo como MP3.</div>

    <form class="search" onsubmit={run}>
      {#if link}<Link2 size={18} />{:else}<Search size={18} />{/if}
      <input type="search" enterkeyhint={link ? "go" : "search"} placeholder="Buscar o pegar un link" bind:value={text} />
      {#if loading}<LoaderCircle size={18} class="spin" />
      {:else if text}<button type="button" class="tap" aria-label="Borrar" onclick={() => (text = "")}><X size={18} /></button>{/if}
    </form>

    {#if active.length || done}
      <div class="downloads">
        {#each active as d (d.id)}
          <div class="job">
            {#if d.status === "error"}<CircleAlert size={18} class="bad" />
            {:else if d.status === "done"}<Check size={18} class="ok" />
            {:else}<Download size={18} />{/if}
            <div class="job-text">
              <div class="ellipsis">{d.title}</div>
              <div class="job-sub ellipsis">{d.error ?? label(d.status, d.item)}</div>
              {#if d.status === "downloading" || d.status === "converting"}
                <div class="bar"><div style:width="{Math.round(d.progress * 100)}%"></div></div>
              {/if}
            </div>
          </div>
        {/each}
        <div class="downloads-foot">
          {#if done}<span>{done === 1 ? "1 bajada" : `${done} bajadas`} en Music/Haze</span>{/if}
          <button class="chip press" onclick={() => api.clearDownloads()}>Limpiar</button>
        </div>
      </div>
    {/if}

    {#if error}<div class="error">{error}</div>{/if}
    {#if searched && !loading && !error}
      <div class="page-sub results">{results.length} resultados para "{searched}"</div>
    {/if}
  </div>
  {#if !searched && !loading && !active.length}
    <div class="hint">
      <TvMinimalPlay size={40} strokeWidth={1.4} />
      <p>Mantené apretado un resultado para bajarlo. La primera búsqueda tarda un poco más: Haze prepara yt-dlp.</p>
    </div>
  {/if}
{/snippet}

<SongList songs={results} context={`YouTube: ${searched}`} {header} />

<style>
  .search {
    display: flex;
    align-items: center;
    gap: 10px;
    height: 46px;
    padding: 0 14px;
    margin-top: 14px;
    border-radius: calc(14px * var(--rs));
    background: rgb(var(--ov) / 0.07);
    border: var(--border) solid var(--line);
    color: var(--text-2);
  }
  .search input {
    flex: 1;
    min-width: 0;
    background: none;
    border: 0;
    padding: 0;
    color: var(--text);
  }
  .search input::-webkit-search-cancel-button {
    display: none;
  }
  .downloads {
    margin-top: 14px;
    padding: 6px 12px;
    border-radius: calc(14px * var(--rs));
    border: var(--border) solid var(--line);
    background: rgb(var(--ov) / 0.04);
  }
  .job {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 8px 0;
    color: var(--text-2);
  }
  .job :global(.ok) {
    color: var(--ok);
  }
  .job :global(.bad) {
    color: var(--danger);
  }
  .job-text {
    flex: 1;
    min-width: 0;
    color: var(--text);
    font-size: 14px;
  }
  .job-sub {
    font-size: 12.5px;
    color: var(--text-3);
  }
  .bar {
    height: 3px;
    margin-top: 6px;
    border-radius: calc(3px * var(--rs));
    background: var(--line);
    overflow: hidden;
  }
  .bar div {
    height: 100%;
    background: var(--accent);
    transition: width 0.3s;
  }
  .downloads-foot {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
    padding: 6px 0;
    font-size: 13px;
    color: var(--text-3);
  }
  .chip {
    margin-left: auto;
    height: 30px;
    padding: 0 12px;
    border-radius: var(--round);
    font-size: 13px;
    color: var(--text-2);
    border: var(--border) solid var(--line);
  }
  .error {
    margin-top: 12px;
    color: var(--danger);
    font-size: 13.5px;
  }
  .results {
    margin: 14px 0 0;
  }
  .hint {
    display: grid;
    justify-items: center;
    gap: 8px;
    padding: 48px 40px;
    color: var(--text-3);
    text-align: center;
  }
</style>
