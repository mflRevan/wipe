<script lang="ts">
  import { browser } from '$app/environment';
  import { Plus } from 'lucide-svelte';
  import Column from './Column.svelte';
  import TrashZone from './TrashZone.svelte';
  import {
    board,
    rewinding,
    moveTicket,
    deleteTicket,
    createList,
    renameList,
    moveList,
    deleteList
  } from '$lib/stores/board';
  import type { List, Ticket } from '$lib/types';

  let { onopen, onadd }: { onopen: (t: Ticket) => void; onadd: (id: string, name: string) => void } =
    $props();

  const reduced = browser && matchMedia('(prefers-reduced-motion: reduce)').matches;
  const flipMs = reduced ? 0 : 150;

  let cols = $state<List[]>([]);
  // Deliberately a plain (untracked) local: the sync effect below must NOT re-run
  // the instant a drag ends, or it would rebuild `cols` from the not-yet-updated
  // store and snap the just-dropped card back to its origin. The effect still
  // re-runs when `$board` itself changes (the ~0.5s poll confirms the move).
  let dragging = false;
  // A SEPARATE reactive flag purely for drag-affordance styling (bigger drop
  // zones + the target-list glow). It's intentionally not read by the sync effect,
  // so toggling it on drop can't trigger the snap-back that `dragging` guards.
  let dragActive = $state(false);

  // Inline "+ Add list" affordance.
  let addingList = $state(false);
  let newListName = $state('');

  function submitList() {
    const v = newListName.trim();
    if (v) void createList(v);
    newListName = '';
    addingList = false;
  }

  function handleMove(listId: string, dir: -1 | 1) {
    const idx = cols.findIndex((c) => c.list === listId);
    if (idx === -1) return;
    const target = idx + dir;
    if (target < 0 || target >= cols.length) return;
    void moveList(listId, target);
  }

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
    dragActive = true;
    const col = colById(listId);
    if (col) col.tickets = items;
  }

  function handleFinalize(
    listId: string,
    items: Ticket[],
    info: { id: string; trigger: string }
  ) {
    const col = colById(listId);
    if (col) col.tickets = items;
    dragging = false;
    dragActive = false;
    // Persist only from the destination zone (covers same-list reorders too).
    // `cols` already reflects the drop; because `dragging` is untracked the sync
    // effect won't revert it, and the ~0.5s poll confirms the move server-side.
    if (info.trigger === 'droppedIntoZone') {
      const pos = items.findIndex((t) => t.id === info.id);
      if (pos !== -1) void moveTicket(info.id, listId, pos);
    }
  }
</script>

<div class="board wp-scroll">
  {#each cols as col, i (col.list)}
    <Column
      listId={col.list}
      name={col.name}
      tickets={col.tickets}
      {flipMs}
      {dragActive}
      dragDisabled={$rewinding}
      canMoveLeft={i > 0}
      canMoveRight={i < cols.length - 1}
      {onopen}
      {onadd}
      onmove={handleMove}
      onrename={(id, name) => renameList(id, name)}
      ondelete={(id) => deleteList(id)}
      onconsider={handleConsider}
      onfinalize={handleFinalize}
    />
  {/each}

  {#if !$rewinding}
    <div class="addcol">
      {#if addingList}
        <!-- svelte-ignore a11y_autofocus -->
        <input
          class="addcol-input"
          autofocus
          placeholder="List name"
          bind:value={newListName}
          onblur={submitList}
          onkeydown={(e) => {
            if (e.key === 'Enter') submitList();
            else if (e.key === 'Escape') {
              newListName = '';
              addingList = false;
            }
          }}
        />
      {:else}
        <button class="addcol-btn" onclick={() => (addingList = true)}>
          <Plus size={16} /> Add list
        </button>
      {/if}
    </div>
  {/if}

  {#if cols.length === 0 && $rewinding}
    <div class="empty">This board has no lists.</div>
  {/if}
</div>

<!-- Drag a card here to delete it. Hidden unless a drag is in progress, and only
     while the board is live (not when viewing history). -->
{#if !$rewinding}
  <TrashZone {dragActive} {flipMs} ondelete={(id) => deleteTicket(id)} />
{/if}

<style>
  .board {
    display: flex;
    gap: 12px;
    height: 100%;
    padding-bottom: 8px;
    overflow-x: auto;
    /* Columns size to their contents and grow with the cards they hold (rather
       than stretching to the board floor). Each column caps at the board height
       and scrolls internally; its drop zone keeps a comfortable min-height so
       even an empty list is an easy, reliable drop target. */
    align-items: flex-start;
  }
  .empty {
    color: var(--wp-text-muted);
    font-size: 14px;
    padding: 24px;
  }
  .addcol {
    width: 280px;
    flex: none;
    align-self: flex-start;
  }
  .addcol-btn {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    width: 100%;
    padding: 12px;
    border: 1px dashed var(--wp-border-strong);
    border-radius: var(--wp-r-lg);
    background: none;
    color: var(--wp-text-muted);
    font-size: 13px;
    font-weight: 500;
    cursor: pointer;
    transition: all var(--wp-fast) var(--wp-ease);
  }
  .addcol-btn:hover {
    background: var(--wp-surface);
    color: var(--wp-text);
    border-color: var(--wp-accent);
  }
  .addcol-input {
    width: 100%;
    height: 40px;
    padding: 0 12px;
    border-radius: var(--wp-r-lg);
    border: 1px solid var(--wp-border-strong);
    background: var(--wp-card);
    color: var(--wp-text);
    font-size: 13px;
  }
</style>
