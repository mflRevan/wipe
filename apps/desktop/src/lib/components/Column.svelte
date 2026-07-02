<script lang="ts">
  import { dndzone, SHADOW_ITEM_MARKER_PROPERTY_NAME, type DndEvent } from 'svelte-dnd-action';
  import { Plus } from 'lucide-svelte';
  import Card from './Card.svelte';
  import type { Ticket } from '$lib/types';

  let {
    listId,
    name,
    tickets,
    flipMs,
    dragDisabled,
    onopen,
    onadd,
    onconsider,
    onfinalize
  }: {
    listId: string;
    name: string;
    tickets: Ticket[];
    flipMs: number;
    dragDisabled: boolean;
    onopen: (t: Ticket) => void;
    onadd: (listId: string, name: string) => void;
    onconsider: (listId: string, items: Ticket[]) => void;
    onfinalize: (listId: string, items: Ticket[], info: { id: string; trigger: string }) => void;
  } = $props();

  const marker = SHADOW_ITEM_MARKER_PROPERTY_NAME;
</script>

<section class="column">
  <header class="col-head">
    <span class="col-name">{name}</span>
    <span class="count">{tickets.length}</span>
    {#if !dragDisabled}
      <button class="add" aria-label="Add card" onclick={() => onadd(listId, name)}>
        <Plus size={15} />
      </button>
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

    {#if tickets.length === 0 && !dragDisabled}
      <button class="empty-add" onclick={() => onadd(listId, name)}>
        <Plus size={14} /> Add a card
      </button>
    {/if}
  </div>
</section>

<style>
  .column {
    display: flex;
    flex-direction: column;
    width: 300px;
    flex: none;
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
  .col-body {
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 4px 8px 12px;
    overflow-y: auto;
    min-height: 40px;
    flex: 1;
  }
  .item {
    position: relative;
  }
  .placeholder {
    height: 64px;
    border: 1px dashed var(--wp-border-strong);
    border-radius: var(--wp-r-md);
    background: color-mix(in srgb, var(--wp-accent) 6%, transparent);
  }
  .empty-add {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 6px;
    padding: 12px;
    border: 1px dashed var(--wp-border);
    border-radius: var(--wp-r-md);
    background: none;
    color: var(--wp-text-subtle);
    font-size: 12px;
    cursor: pointer;
  }
  .empty-add:hover {
    background: var(--wp-elevated);
    color: var(--wp-text);
  }
</style>
