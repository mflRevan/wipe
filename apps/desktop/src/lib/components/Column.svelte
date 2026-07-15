<script lang="ts">
  import { dndzone, SHADOW_ITEM_MARKER_PROPERTY_NAME, type DndEvent } from 'svelte-dnd-action';
  import { flip } from 'svelte/animate';
  import { Plus, MoreHorizontal, ChevronLeft, ChevronRight, Pencil, Trash2 } from 'lucide-svelte';
  import Card from './Card.svelte';
  import Popover from './ui/Popover.svelte';
  import type { Ticket } from '$lib/types';

  let {
    listId,
    name,
    tickets,
    flipMs,
    dragDisabled,
    dragActive = false,
    canMoveLeft = false,
    canMoveRight = false,
    onopen,
    onadd,
    onmove,
    onrename,
    ondelete,
    onconsider,
    onfinalize,
    ondragel
  }: {
    listId: string;
    name: string;
    tickets: Ticket[];
    flipMs: number;
    dragDisabled: boolean;
    dragActive?: boolean;
    canMoveLeft?: boolean;
    canMoveRight?: boolean;
    onopen: (t: Ticket) => void;
    onadd: (listId: string, name: string) => void;
    onmove?: (listId: string, dir: -1 | 1) => void;
    onrename?: (listId: string, name: string) => void;
    ondelete?: (listId: string) => void;
    onconsider: (listId: string, items: Ticket[]) => void;
    onfinalize: (listId: string, items: Ticket[], info: { id: string; trigger: string }) => void;
    ondragel?: (el: HTMLElement) => void;
  } = $props();

  const marker = SHADOW_ITEM_MARKER_PROPERTY_NAME;

  // svelte-dnd-action places the drop placeholder (an item carrying the shadow
  // marker) into whichever zone the card is currently over - so exactly one
  // column holds it at a time. That's the list a release would land in; glow only
  // that one. (dropTargetStyle can't be used for this: it styles ALL valid zones.)
  let isTarget = $derived(dragActive && tickets.some((t) => marker in t));

  let renaming = $state(false);
  let renameDraft = $state('');

  function startRename() {
    renameDraft = name;
    renaming = true;
  }
  function commitRename() {
    const v = renameDraft.trim();
    renaming = false;
    if (v && v !== name) onrename?.(listId, v);
  }
</script>

<section class="column" class:target={isTarget}>
  <header class="col-head">
    {#if renaming}
      <!-- svelte-ignore a11y_autofocus -->
      <input
        class="rename"
        autofocus
        bind:value={renameDraft}
        onblur={commitRename}
        onkeydown={(e) => {
          if (e.key === 'Enter') e.currentTarget.blur();
          else if (e.key === 'Escape') renaming = false;
        }}
      />
    {:else}
      <span class="col-name">{name}</span>
      <span class="count">{tickets.length}</span>
    {/if}
    {#if !dragDisabled}
      <button class="add" aria-label="Add card" onclick={() => onadd(listId, name)}>
        <Plus size={15} />
      </button>
      <Popover align="end" width="180px">
        {#snippet trigger({ toggle })}
          <button class="add" aria-label="List options" onclick={toggle}>
            <MoreHorizontal size={15} />
          </button>
        {/snippet}
        {#snippet children({ close })}
          <button
            class="mi"
            onclick={() => {
              close();
              startRename();
            }}><Pencil size={14} /> Rename list</button
          >
          <button
            class="mi"
            disabled={!canMoveLeft}
            onclick={() => {
              close();
              onmove?.(listId, -1);
            }}><ChevronLeft size={14} /> Move left</button
          >
          <button
            class="mi"
            disabled={!canMoveRight}
            onclick={() => {
              close();
              onmove?.(listId, 1);
            }}><ChevronRight size={14} /> Move right</button
          >
          <div class="mdiv"></div>
          <button
            class="mi danger"
            title="Delete list"
            onclick={() => {
              close();
              // Deleting a non-empty list also deletes its tickets - confirm first
              // so a menu misclick can't wipe a column of work.
              if (
                tickets.length === 0 ||
                confirm(
                  `Delete "${name}" and its ${tickets.length} ticket${tickets.length === 1 ? '' : 's'}? This cannot be undone.`
                )
              ) {
                ondelete?.(listId);
              }
            }}><Trash2 size={14} /> Delete list</button
          >
        {/snippet}
      </Popover>
    {/if}
  </header>

  <div
    class="col-body wp-scroll"
    class:dragging={dragActive}
    use:dndzone={{
      items: tickets,
      flipDurationMs: flipMs,
      dragDisabled,
      // Disable the library's built-in target styling: it highlights EVERY valid
      // zone. We glow just the hovered column ourselves via `isTarget` below.
      dropTargetStyle: {},
      transformDraggedElement: (el?: HTMLElement) => {
        if (el) {
          el.style.boxShadow = 'var(--wp-shadow-lift)';
          el.style.borderRadius = 'var(--wp-r-md)';
          el.style.cursor = 'grabbing';
          // Smoothly scale when the pointer is over the trash (Board drives this).
          el.style.transition = 'transform 0.12s var(--wp-ease)';
          el.style.transformOrigin = 'center';
          ondragel?.(el);
        }
      }
    }}
    onconsider={(e: CustomEvent<DndEvent<Ticket>>) => onconsider(listId, e.detail.items)}
    onfinalize={(e: CustomEvent<DndEvent<Ticket>>) =>
      onfinalize(listId, e.detail.items, e.detail.info)}
  >
    {#each tickets as ticket (ticket.id)}
      <div class="item" animate:flip={{ duration: flipMs }}>
        {#if marker in ticket}
          <!-- The drop slot: render the dragged card hidden so the gap is EXACTLY
               its size, with a dashed outline showing where it will land. -->
          <div class="slot">
            <Card {ticket} {onopen} />
          </div>
        {:else}
          <Card {ticket} {onopen} />
        {/if}
      </div>
    {/each}
  </div>

  <!-- Kept OUTSIDE the dndzone: svelte-dnd-action treats every direct child of
       the zone as a draggable item, so an add-card control living inside it
       would be grabbable and corrupt the item list on drag. -->
  {#if !dragDisabled}
    <button class="add-card" onclick={() => onadd(listId, name)}>
      <Plus size={14} /> Add a card
    </button>
  {/if}
</section>

<style>
  .column {
    display: flex;
    flex-direction: column;
    width: 300px;
    flex: none;
    /* Height follows the cards (grows with the list); capped at the board height,
       past which the card area scrolls internally. */
    max-height: 100%;
    background: var(--wp-surface);
    border: 1px solid var(--wp-border);
    border-radius: var(--wp-r-lg);
    transition:
      border-color var(--wp-fast) var(--wp-ease),
      box-shadow var(--wp-fast) var(--wp-ease),
      background var(--wp-fast) var(--wp-ease);
  }
  /* The single list a release would land in - glow it in the accent color. */
  .column.target {
    border-color: color-mix(in srgb, var(--wp-accent) 65%, transparent);
    background: color-mix(in srgb, var(--wp-accent) 6%, var(--wp-surface));
    box-shadow: 0 0 0 1px color-mix(in srgb, var(--wp-accent) 45%, transparent);
  }
  .col-head {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 12px 12px 8px;
  }
  .col-name {
    font-family: var(--wp-font-display);
    font-size: 12px;
    font-weight: 600;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    color: var(--wp-text-muted);
  }
  .count {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    min-width: 20px;
    height: 18px;
    padding: 0 6px;
    border-radius: var(--wp-r-pill);
    background: var(--wp-card);
    border: 1px solid var(--wp-border);
    font-size: 11px;
    font-family: var(--wp-font-mono);
    color: var(--wp-text-subtle);
  }
  .add {
    margin-left: auto;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 24px;
    height: 24px;
    border-radius: var(--wp-r-sm);
    border: none;
    background: none;
    color: var(--wp-text-muted);
    cursor: pointer;
    transition: all var(--wp-fast) var(--wp-ease);
  }
  .add:hover {
    background: var(--wp-elevated);
    color: var(--wp-text);
  }
  .rename {
    flex: 1;
    height: 24px;
    padding: 0 8px;
    border-radius: var(--wp-r-sm);
    border: 1px solid var(--wp-border-strong);
    background: var(--wp-card);
    color: var(--wp-text);
    font-family: var(--wp-font-display);
    font-size: 12px;
    font-weight: 600;
    letter-spacing: 0.04em;
    text-transform: uppercase;
  }
  .mi {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
    padding: 7px 8px;
    border: none;
    background: none;
    color: var(--wp-text);
    border-radius: var(--wp-r-sm);
    cursor: pointer;
    font-size: 13px;
    text-align: left;
  }
  .mi:hover:not(:disabled) {
    background: var(--wp-elevated);
  }
  .mi:disabled {
    opacity: 0.4;
    cursor: not-allowed;
  }
  .mi.danger {
    color: var(--wp-error);
  }
  .mdiv {
    height: 1px;
    background: var(--wp-border);
    margin: 4px 0;
  }
  .col-body {
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 4px 8px 14px;
    overflow-y: auto;
    /* Comfortable floor so an empty list is still an easy drop target, without
       stretching the column to the board floor. `flex: 0 1 auto` lets the area
       shrink and scroll when the column hits its height cap, but never grow past
       its contents. */
    min-height: 52px;
    flex: 0 1 auto;
    transition: min-height var(--wp-base) var(--wp-ease);
  }
  /* While a card is being dragged anywhere on the board, grow every list's drop
     zone so it's a big, forgiving target - you can hover a short list (or the
     space below its cards) and it still registers as the landing list instead of
     snapping the card back. */
  .col-body.dragging {
    min-height: 140px;
  }
  .item {
    position: relative;
  }
  /* Drop slot: exactly the size of the card being dragged (its hidden card sets the
     height), shown as a dashed accent outline so you see precisely where it lands. */
  .slot {
    position: relative;
    border-radius: var(--wp-r-md);
    background: color-mix(in srgb, var(--wp-accent) 10%, transparent);
    outline: 2px dashed color-mix(in srgb, var(--wp-accent) 55%, transparent);
    outline-offset: -2px;
  }
  .slot > :global(.card) {
    visibility: hidden;
  }
  .add-card {
    display: flex;
    align-items: center;
    gap: 6px;
    margin: 0 8px 8px;
    padding: 8px 10px;
    border: none;
    border-radius: var(--wp-r-md);
    background: none;
    color: var(--wp-text-subtle);
    font-size: 13px;
    cursor: pointer;
    transition: all var(--wp-fast) var(--wp-ease);
  }
  .add-card:hover {
    background: var(--wp-elevated);
    color: var(--wp-text);
  }
</style>
