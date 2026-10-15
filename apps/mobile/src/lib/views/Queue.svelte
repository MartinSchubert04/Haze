<script lang="ts">
  import { time } from "@haze/ui/format";
  import { api, type QueueView } from "../api";
  import { store } from "../store.svelte";
  import { longpress } from "../longpress";
  import Cover from "../components/Cover.svelte";

  let q = $state<QueueView | null>(null);

  $effect(() => {
    store.player.queueVersion;
    api.queue().then((v) => (q = v));
  });

  const cur = $derived(q?.currentIndex ?? -1);
  const pendingEphemeral = $derived(q ? q.entries.slice(cur + 1).some((e) => e.ephemeral) : false);
</script>

<div class="scroller">
  <div class="page">
    <h1 class="page-title">Cola</h1>
    <div class="page-sub">{q?.context || "Nada sonando"}</div>
    {#if pendingEphemeral}
      <button class="pill" onclick={() => api.queueAction("clear")}>Vaciar lo encolado</button>
    {/if}
  </div>

  {#if q && q.entries.length}
    {#each q.entries as e, i (e.uid)}
      {#if i === cur + 1}<div class="label">A continuación</div>{/if}
      <button
        class="row press"
        class:current={i === cur}
        class:played={i < cur}
        onclick={() => api.queueAction("jump", i)}
        use:longpress={() => store.open({ kind: "song", song: e.song, queueIndex: i })}
      >
        <Cover song={e.song} size={46} radius={8} />
        <div class="text">
          <div class="title ellipsis">{e.song.title}</div>
          <div class="sub ellipsis">
            {#if e.ephemeral}<span class="tag">Encolada</span>{/if}
            {e.song.artist || "Artista desconocido"}
          </div>
        </div>
        <div class="dur">{time(e.song.durationMs / 1000)}</div>
      </button>
    {/each}
  {:else}
    <div class="empty"><h3>La cola está vacía</h3><p>Tocá una canción para empezar.</p></div>
  {/if}
  <div class="spacer-bottom"></div>
</div>

<style>
  .scroller {
    height: 100%;
    overflow-y: auto;
  }
  .pill {
    margin-bottom: 6px;
  }
  .label {
    padding: 14px 16px 6px;
    font-size: 12.5px;
    font-weight: 600;
    letter-spacing: 0.04em;
    text-transform: uppercase;
    color: var(--text-3);
  }
  .row {
    width: 100%;
    height: 64px;
    display: flex;
    align-items: center;
    gap: 14px;
    padding: 0 16px;
    text-align: left;
  }
  .played {
    opacity: 0.45;
  }
  .text {
    flex: 1;
    min-width: 0;
  }
  .title {
    font-weight: 550;
  }
  .current .title {
    color: var(--accent);
  }
  .sub {
    font-size: 13px;
    color: var(--text-2);
    margin-top: 2px;
  }
  .tag {
    color: var(--accent);
    margin-right: 4px;
  }
  .dur {
    font-size: 12.5px;
    color: var(--text-3);
    font-variant-numeric: tabular-nums;
  }
</style>
