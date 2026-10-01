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

  // The board's lists, as RAW state replaced immutably: deep proxies over every
  // ticket's comments and activity made each drag step and board refresh walk
  // the whole board. (Search hides non-matching cards in place - see Column - so
  // the drag library always works on the full lists.)
  let cols = $state.raw<List[]>([]);

  // --- trash-on-drag (pointer tracked, not a drop zone) --------------------
  // The dragged card element (captured from svelte-dnd-action) and the trash bin
  // element, so we can hit-test the POINTER against the bin and scale the card.
  let draggedEl: HTMLElement | null = null;
  let trashBinEl = $state<HTMLElement | null>(null);
  // Reactive: pointer is currently over the trash (release would delete).
  let overTrash = $state(false);

  // While a card is being dragged, track the raw pointer position: if it's over
  // the bin, shrink the dragged card to 0.7x (a release deletes it); else 1x.
  $effect(() => {
    if (!dragActive) return;
    const onMove = (e: PointerEvent) => {
      const el = trashBinEl;
      if (!el) {
        overTrash = false;
        return;
      }
      const r = el.getBoundingClientRect();
      const pad = 36; // generous margin so the bin is easy to reach
      overTrash =
        e.clientX >= r.left - pad &&
        e.clientX <= r.right + pad &&
        e.clientY >= r.top - pad &&
        e.clientY <= r.bottom + pad;
      if (draggedEl) draggedEl.style.transform = overTrash ? 'scale(0.7)' : 'scale(1)';
    };
    window.addEventListener('pointermove', onMove);
    return () => {
      window.removeEventListener('pointermove', onMove);
      overTrash = false;
    };
  });
  // Deliberately a plain (untracked) local: the sync effects below must NOT re-run
  // the instant a drag ends, or they would rebuild from the not-yet-updated store
  // and snap the just-dropped card back to its origin. They still re-run when
  // `$board` itself changes (the poll confirms the move).
  let dragging = false;
  // A SEPARATE reactive flag purely for drag-affordance styling (bigger drop
  // zones + the target-list glow). It's intentionally not read by the sync effects,
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
    cols = b.lists;
  });

  function setCol(listId: string, items: Ticket[]) {
    cols = cols.map((l) => (l.list === listId ? { ...l, tickets: items } : l));
  }

  function handleConsider(listId: string, items: Ticket[]) {
    dragging = true;
    dragActive = true;
    setCol(listId, items);
  }

  function handleFinalize(
    listId: string,
    items: Ticket[],
    info: { id: string; trigger: string }
  ) {
    const droppedOnTrash = overTrash;
    dragging = false;
    dragActive = false;
    draggedEl = null;
    // Dropped on the bin: the card leaves every list right now, so it never
    // reappears at its origin while the delete round-trips. This wins even if
    // the release also happened over a column zone.
    if (droppedOnTrash) {
      cols = cols.map((l) => ({ ...l, tickets: l.tickets.filter((t) => t.id !== info.id) }));
      void deleteTicket(info.id);
      return;
    }
    setCol(listId, items);
    // Persist only from the destination zone (covers same-list reorders too).
    // `cols` already reflects the drop; because `dragging` is untracked the sync
    // effect won't revert it, and the poll confirms the move server-side.
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
      {overTrash}
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
      ondragel={(el) => (draggedEl = el)}
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

<!-- Always-visible trash: a click opens the restore panel; during a card drag the
     pointer position (tracked above) decides whether a release deletes. -->
{#if !$rewinding}
  <TrashZone {dragActive} {overTrash} bind:binEl={trashBinEl} />
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
  /* Phones: one list per screen, swiped between with snap points; the next list
     peeks in at the edge so it's obvious there is more. */
  @media (max-width: 700px) {
    .board {
      gap: 10px;
      padding: 0 12px 8px;
      scroll-snap-type: x mandatory;
      scroll-padding: 0 12px;
      -webkit-overflow-scrolling: touch;
    }
    .board > :global(*) {
      scroll-snap-align: start;
    }
    .addcol {
      width: 84vw;
    }
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
