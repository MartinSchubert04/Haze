<script lang="ts">
  import { GripVertical, X, AudioLines } from "@lucide/svelte";
  import { api, type QueueEntry, type QueueView } from "../api";
  import { store } from "../store.svelte";
  import { songMenu } from "../menu.svelte";
  import { time } from "../format";
  import Cover from "../components/Cover.svelte";

  const LIMIT = 200;
  let q = $state<QueueView | null>(null);

  $effect(() => {
    store.player.queueVersion;
    api.queue().then((v) => (q = v));
  });

  const cur = $derived(q?.currentIndex ?? -1);
  const upcoming = $derived(q ? q.entries.map((e, i) => ({ e, i })).filter(({ i }) => i > cur) : []);
  const queued = $derived(upcoming.filter(({ e }) => e.ephemeral));
  const rest = $derived(upcoming.filter(({ e }) => !e.ephemeral));

  let dragFrom = $state<number | null>(null);
  let dragOver = $state<number | null>(null);

  function drop(to: number) {
    if (dragFrom !== null && dragFrom !== to) api.queueAction("move", dragFrom, to);
    dragFrom = dragOver = null;
  }
</script>

{#snippet row(e: QueueEntry, i: number)}
  <div
    class="row"
    class:over={dragOver === i}
    class:dragging={dragFrom === i}
    draggable="true"
    role="listitem"
    ondragstart={(ev) => {
      dragFrom = i;
      ev.dataTransfer?.setData("text/plain", String(i));
    }}
    ondragover={(ev) => {
      ev.preventDefault();
      dragOver = i;
    }}
    ondragend={() => (dragFrom = dragOver = null)}
    ondrop={(ev) => {
      ev.preventDefault();
      drop(i);
    }}
    ondblclick={() => api.queueAction("jump", i)}
    oncontextmenu={(ev) => songMenu(ev, e.song, () => api.queueAction("jump", i))}
  >
    <span class="grip"><GripVertical size={16} /></span>
    <button class="main" onclick={() => api.queueAction("jump", i)}>
      <Cover song={e.song} size={40} radius={8} />
      <div class="txt">
        <div class="t ellipsis">{e.song.title}</div>
        <div class="a ellipsis">{e.song.artist || "Desconocido"}</div>
      </div>
    </button>
    <span class="dur">{e.song.durationMs ? time(e.song.durationMs / 1000) : ""}</span>
    <button class="icon-btn rm" title="Sacar de la cola" onclick={() => api.queueAction("remove", i)}><X size={16} /></button>
  </div>
{/snippet}

<div class="scroll">
  <div class="page">
    <h1 class="page-title">Cola</h1>
    <div class="page-sub">Arrastrá para reordenar. Lo que encolás a mano suena antes de seguir con {q?.context || "la lista"}.</div>

    {#if !q || q.entries.length === 0}
      <div class="empty">
        <h3>No hay nada sonando</h3>
        <p>Tocá una canción, o usá "Agregar a la cola" desde el menú de cualquier tema.</p>
      </div>
    {:else}
      {#if cur >= 0}
        {@const now = q.entries[cur]}
        <div class="label">Sonando</div>
        <div class="now">
          <Cover song={now.song} size={56} radius={10} />
          <div class="txt">
            <div class="t ellipsis accent">{now.song.title}</div>
            <div class="a ellipsis">{now.song.artist || "Desconocido"}</div>
          </div>
          <span class="eq" class:paused={!store.player.playing}><AudioLines size={20} /></span>
        </div>
      {/if}

      {#if queued.length}
        <div class="label row-label">
          <span>En cola</span>
          <button class="link" onclick={() => api.queueAction("clear")}>Vaciar</button>
        </div>
        <div class="list" role="list">
          {#each queued as { e, i } (e.uid)}{@render row(e, i)}{/each}
        </div>
      {/if}

      {#if rest.length}
        <div class="label">A continuación de {q.context || "la lista"}</div>
        <div class="list" role="list">
          {#each rest.slice(0, LIMIT) as { e, i } (e.uid)}{@render row(e, i)}{/each}
        </div>
        {#if rest.length > LIMIT}<div class="more">y {rest.length - LIMIT} más</div>{/if}
      {/if}
    {/if}
  </div>
</div>

<style>
  .scroll {
    height: 100%;
    overflow-y: auto;
  }
  .label {
    margin: 22px 0 8px;
    font-size: 12px;
    font-weight: 600;
    color: var(--text-3);
    text-transform: uppercase;
    letter-spacing: 0.06em;
  }
  .row-label {
    display: flex;
    justify-content: space-between;
    align-items: center;
  }
  .link {
    text-transform: none;
    letter-spacing: 0;
    color: var(--accent);
    font-weight: 560;
    font-size: 12.5px;
  }
  .now {
    display: flex;
    align-items: center;
    gap: 14px;
    padding: 10px;
    border-radius: var(--radius-lg);
    background: rgba(255, 255, 255, 0.07);
  }
  .accent {
    color: var(--accent);
  }
  .eq {
    margin-left: auto;
    color: var(--accent);
    padding-right: 8px;
  }
  .eq :global(svg) {
    animation: pulse 1.1s ease-in-out infinite;
  }
  .eq.paused :global(svg) {
    animation: none;
  }
  @keyframes pulse {
    50% {
      transform: scaleY(0.6);
    }
  }
  .list {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .row {
    display: grid;
    grid-template-columns: 24px minmax(0, 1fr) 50px 34px;
    align-items: center;
    gap: 8px;
    padding: 6px 8px 6px 4px;
    border-radius: var(--radius);
    border-top: 2px solid transparent;
    transition: background 0.12s;
  }
  .row:hover {
    background: rgba(255, 255, 255, 0.06);
  }
  .row.over {
    border-top-color: var(--accent);
  }
  .row.dragging {
    opacity: 0.4;
  }
  .grip {
    color: var(--text-3);
    cursor: grab;
    display: grid;
    place-items: center;
  }
  .main {
    display: flex;
    align-items: center;
    gap: 12px;
    text-align: left;
    min-width: 0;
  }
  .txt {
    min-width: 0;
  }
  .t {
    font-weight: 540;
  }
  .a {
    color: var(--text-2);
    font-size: 12.5px;
    margin-top: 2px;
  }
  .dur {
    color: var(--text-3);
    font-size: 12.5px;
    text-align: right;
    font-variant-numeric: tabular-nums;
  }
  .rm {
    opacity: 0;
  }
  .row:hover .rm {
    opacity: 1;
  }
  .more {
    color: var(--text-3);
    padding: 12px;
    font-size: 13px;
  }
</style>
