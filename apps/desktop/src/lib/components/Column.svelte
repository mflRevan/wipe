<script lang="ts">
  import { dndzone, SHADOW_ITEM_MARKER_PROPERTY_NAME, type DndEvent } from 'svelte-dnd-action';
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
    canMoveLeft = false,
    canMoveRight = false,
    onopen,
    onadd,
    onmove,
    onrename,
    ondelete,
    onconsider,
    onfinalize
  }: {
    listId: string;
    name: string;
    tickets: Ticket[];
    flipMs: number;
    dragDisabled: boolean;
    canMoveLeft?: boolean;
    canMoveRight?: boolean;
    onopen: (t: Ticket) => void;
    onadd: (listId: string, name: string) => void;
    onmove?: (listId: string, dir: -1 | 1) => void;
    onrename?: (listId: string, name: string) => void;
    ondelete?: (listId: string) => void;
    onconsider: (listId: string, items: Ticket[]) => void;
    onfinalize: (listId: string, items: Ticket[], info: { id: string; trigger: string }) => void;
  } = $props();

  const marker = SHADOW_ITEM_MARKER_PROPERTY_NAME;

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

<section class="column">
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
            disabled={tickets.length > 0}
            title={tickets.length > 0 ? 'List must be empty' : 'Delete list'}
            onclick={() => {
              close();
              ondelete?.(listId);
            }}><Trash2 size={14} /> Delete list</button
          >
        {/snippet}
      </Popover>
    {/if}
  </header>

  <div
    class="col-body wp-scroll"
    use:dndzone={{
      items: tickets,
      flipDurationMs: flipMs,
      dragDisabled,
      dropTargetStyle: {},
      transformDraggedElement: (el?: HTMLElement) => {
        if (el) {
          el.style.boxShadow = 'var(--wp-shadow-lift)';
          el.style.borderRadius = 'var(--wp-r-md)';
          el.style.cursor = 'grabbing';
        }
      }
    }}
    onconsider={(e: CustomEvent<DndEvent<Ticket>>) => onconsider(listId, e.detail.items)}
    onfinalize={(e: CustomEvent<DndEvent<Ticket>>) =>
      onfinalize(listId, e.detail.items, e.detail.info)}
  >
    {#each tickets as ticket (ticket.id)}
      <div class="item">
        {#if marker in ticket}
          <div class="placeholder"></div>
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
  }
  .item {
    position: relative;
  }
  .placeholder {
    height: 64px;
    border: 2px dashed color-mix(in srgb, var(--wp-accent) 45%, transparent);
    border-radius: var(--wp-r-md);
    background: color-mix(in srgb, var(--wp-accent) 9%, transparent);
    transition: all var(--wp-fast) var(--wp-ease);
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
