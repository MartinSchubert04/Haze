<script lang="ts">
  import { Music } from "@lucide/svelte";
  import { coverUrl, type Song } from "../api";

  let {
    song,
    size = 40,
    radius = 8,
    round = false,
  }: { song: Song | null | undefined; size?: number | string; radius?: number; round?: boolean } = $props();

  let failed = $state(false);
  const src = $derived(coverUrl(song));
  $effect(() => {
    src;
    failed = false;
  });
  const dim = $derived(typeof size === "number" ? `${size}px` : size);
</script>

<div class="cover" style:width={dim} style:height={dim} style:border-radius={round ? "50%" : `${radius}px`}>
  {#if src && !failed}
    <img {src} alt="" loading="lazy" decoding="async" draggable="false" onerror={() => (failed = true)} />
  {:else}
    <Music size={typeof size === "number" ? Math.max(14, size * 0.4) : 48} strokeWidth={1.6} />
  {/if}
</div>

<style>
  .cover {
    flex: none;
    display: grid;
    place-items: center;
    overflow: hidden;
    background: rgba(255, 255, 255, 0.07);
    color: var(--text-3);
  }
  img {
    width: 100%;
    height: 100%;
    object-fit: cover;
    display: block;
  }
</style>
