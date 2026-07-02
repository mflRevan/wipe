<script lang="ts">
  import { browser } from '$app/environment';
  import Column from './Column.svelte';
  import { board, rewinding, moveTicket } from '$lib/stores/board';
  import type { List, Ticket } from '$lib/types';

  let { onopen, onadd }: { onopen: (t: Ticket) => void; onadd: (id: string, name: string) => void } =
    $props();

  const reduced = browser && matchMedia('(prefers-reduced-motion: reduce)').matches;
  const flipMs = reduced ? 0 : 180;

  let cols = $state<List[]>([]);
  let dragging = false;

  // Sync local columns from the store whenever the board changes (not mid-drag).
  $effect(() => {
    const b = $board;
    if (!b || dragging) return;
    cols = b.lists.map((l) => ({ ...l, tickets: [...l.tickets] }));
  });

  function colById(id: string): List | undefined {
    return cols.find((c) => c.list === id);
  }

  function handleConsider(listId: string, items: Ticket[]) {
    dragging = true;
    const col = colById(listId);
    if (col) col.tickets = items;
  }

  async function handleFinalize(
    listId: string,
    items: Ticket[],
    info: { id: string; trigger: string }
  ) {
    const col = colById(listId);
    if (col) col.tickets = items;
    dragging = false;
    // Persist only from the destination zone (covers same-list reorders too).
    if (info.trigger === 'droppedIntoZone') {
      const pos = items.findIndex((t) => t.id === info.id);
      if (pos !== -1) await moveTicket(info.id, listId, pos);
    }
  }
</script>

<div class="board wp-scroll">
  {#each cols as col (col.list)}
    <Column
      listId={col.list}
      name={col.name}
      tickets={col.tickets}
      {flipMs}
      dragDisabled={$rewinding}
      {onopen}
      {onadd}
      onconsider={handleConsider}
      onfinalize={handleFinalize}
    />
  {/each}
  {#if cols.length === 0}
    <div class="empty">This board has no lists.</div>
  {/if}
</div>

<style>
  .board {
    display: flex;
    gap: 12px;
    height: 100%;
    padding-bottom: 8px;
    overflow-x: auto;
    align-items: flex-start;
  }
  .empty {
    color: var(--wp-text-muted);
    font-size: 14px;
    padding: 24px;
  }
</style>
