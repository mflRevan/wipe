<script lang="ts">
  // A bell that watches/unwatches one ticket, list or forum thread.
  import { Bell, BellRing } from 'lucide-svelte';
  import { watches, toggleWatch, isWatching } from '$lib/stores/notify';

  let {
    kind,
    id,
    label = true
  }: { kind: 'tickets' | 'lists' | 'threads'; id: string; label?: boolean } = $props();

  let on = $derived(isWatching($watches, kind, id));
  let noun = $derived(kind === 'tickets' ? 'ticket' : kind === 'lists' ? 'list' : 'thread');
</script>

<button
  class="watch"
  class:on
  onclick={(e) => {
    e.stopPropagation();
    toggleWatch(kind, id);
  }}
  title={on
    ? `Watching this ${noun} - click to stop notifications`
    : `Watch this ${noun}: get notified when others change it`}
  aria-pressed={on}
>
  {#if on}<BellRing size={13} />{:else}<Bell size={13} />{/if}
  {#if label}<span>{on ? 'Watching' : 'Watch'}</span>{/if}
</button>

<style>
  .watch {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    height: 24px;
    padding: 0 8px;
    border-radius: var(--wp-r-pill);
    border: 1px solid var(--wp-border);
    background: none;
    color: var(--wp-text-muted);
    font-size: 12px;
    cursor: pointer;
    transition: all var(--wp-fast) var(--wp-ease);
  }
  .watch:hover {
    color: var(--wp-text);
    background: var(--wp-elevated);
  }
  .watch.on {
    color: var(--wp-accent);
    border-color: color-mix(in srgb, var(--wp-accent) 55%, transparent);
    background: color-mix(in srgb, var(--wp-accent) 10%, transparent);
  }
</style>
