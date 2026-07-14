<script lang="ts">
  import { browser } from '$app/environment';
  import { Plus } from 'lucide-svelte';
  import {
    dndzone,
    SHADOW_ITEM_MARKER_PROPERTY_NAME,
    type DndEvent
  } from 'svelte-dnd-action';
  import { flip } from 'svelte/animate';
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

  // svelte-dnd-action requires each draggable item to carry an `id`; our list
  // objects key on `list`, so the columns fed to the board-level zone get an
  // `id` alias alongside.
  type Col = List & { id: string };
  const marker = SHADOW_ITEM_MARKER_PROPERTY_NAME;

  let cols = $state<Col[]>([]);
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
    cols = b.lists.map((l) => ({ ...l, id: l.list, tickets: [...l.tickets] }));
  });

  // --- list (column) drag-to-reorder ---------------------------------------
  // A board-level dndzone reorders the columns themselves. It uses a distinct
  // `type` so cards (in the columns' default-type zones) can never drop into it
  // and vice-versa; grabbing a card starts a card drag, grabbing the column
  // header starts a column drag.
  function handleListConsider(e: CustomEvent<DndEvent<Col>>) {
    dragging = true;
    // Never let an empty items array blank the board (a defensive guard against a
    // malformed/interrupted drag); a real reorder always carries every column.
    if (e.detail.items.length) cols = e.detail.items;
  }

  function handleListFinalize(e: CustomEvent<DndEvent<Col>>) {
    if (e.detail.items.length) cols = e.detail.items;
    dragging = false;
    if (e.detail.info.trigger === 'droppedIntoZone') {
      const id = e.detail.info.id;
      const pos = cols.findIndex((c) => c.list === id);
      if (pos !== -1) void moveList(id, pos);
    }
  }

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
  <div
    class="lists"
    use:dndzone={{
      items: cols,
      type: 'column',
      flipDurationMs: flipMs,
      dragDisabled: $rewinding,
      dropTargetStyle: {},
      morphDisabled: true,
      transformDraggedElement: (el?: HTMLElement) => {
        if (el) {
          el.style.boxShadow = 'var(--wp-shadow-lift)';
          el.style.borderRadius = 'var(--wp-r-lg)';
          el.style.cursor = 'grabbing';
        }
      }
    }}
    onconsider={handleListConsider}
    onfinalize={handleListFinalize}
  >
    {#each cols as col, i (col.list)}
      <div class="col-wrap" class:ghost={marker in col} animate:flip={{ duration: flipMs }}>
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
      </div>
    {/each}
  </div>

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
  /* The reorderable row of columns (a nested dndzone). Kept separate from the
     board so the "+ Add list" affordance isn't treated as a draggable item. */
  .lists {
    display: flex;
    gap: 12px;
    align-items: flex-start;
  }
  .col-wrap {
    flex: none;
  }
  /* The column header is the drag handle for reordering lists; a plain click on
     its buttons still fires (a drag only starts once the pointer moves). */
  .col-wrap :global(.col-head) {
    cursor: grab;
  }
  /* The placeholder left where a dragged column will land: hide the real column
     and show a dashed accent outline the same size. */
  .col-wrap.ghost > :global(.column) {
    visibility: hidden;
  }
  .col-wrap.ghost {
    border-radius: var(--wp-r-lg);
    background: color-mix(in srgb, var(--wp-accent) 10%, transparent);
    outline: 2px dashed color-mix(in srgb, var(--wp-accent) 55%, transparent);
    outline-offset: -2px;
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
