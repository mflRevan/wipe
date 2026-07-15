<script lang="ts">
  import Popover from './ui/Popover.svelte';
  import Avatar from './Avatar.svelte';
  import {
    MoreHorizontal,
    Copy,
    Trash2,
    CornerUpRight,
    Flag,
    Tag,
    UserPlus,
    ChevronRight,
    ChevronLeft,
    Check,
    Circle
  } from 'lucide-svelte';
  import {
    board,
    definitions,
    identities,
    moveTicket,
    duplicateTicket,
    deleteTicket,
    updateTicket
  } from '$lib/stores/board';
  import { labelColorFor, priorityColor } from '$lib/utils';
  import type { Ticket } from '$lib/types';

  let { ticket }: { ticket: Ticket } = $props();

  type View = 'root' | 'move' | 'priority' | 'labels' | 'assign';
  let view = $state<View>('root');

  let lists = $derived($board?.lists ?? []);
  let currentList = $derived(lists.find((l) => l.tickets.some((t) => t.id === ticket.id))?.list);

  function toggleLabel(name: string) {
    const labels = ticket.labels.includes(name)
      ? ticket.labels.filter((l) => l !== name)
      : [...ticket.labels, name];
    void updateTicket(ticket.id, { labels });
  }
  function toggleAssignee(id: string) {
    const assignees = ticket.assignees.includes(id)
      ? ticket.assignees.filter((a) => a !== id)
      : [...ticket.assignees, id];
    void updateTicket(ticket.id, { assignees });
  }
</script>

<div class="cm" onclick={(e) => e.stopPropagation()} role="presentation">
  <Popover align="end">
    {#snippet trigger({ toggle, open })}
      <button
        class="cm-btn"
        class:open
        aria-label="Ticket actions"
        title="Actions"
        onclick={(e) => {
          e.stopPropagation();
          view = 'root';
          toggle();
        }}
      >
        <MoreHorizontal size={15} />
      </button>
    {/snippet}

    {#snippet children({ close })}
      {#if view === 'root'}
        <button class="mi" onclick={() => (view = 'move')}>
          <CornerUpRight size={14} /> Move to <ChevronRight size={13} class="mi-chev" />
        </button>
        <button class="mi" onclick={() => (view = 'priority')}>
          <Flag size={14} /> Priority <ChevronRight size={13} class="mi-chev" />
        </button>
        <button class="mi" onclick={() => (view = 'labels')}>
          <Tag size={14} /> Labels <ChevronRight size={13} class="mi-chev" />
        </button>
        <button class="mi" onclick={() => (view = 'assign')}>
          <UserPlus size={14} /> Assign <ChevronRight size={13} class="mi-chev" />
        </button>
        <div class="mdiv"></div>
        <button
          class="mi"
          onclick={() => {
            void duplicateTicket(ticket.id);
            close();
          }}
        >
          <Copy size={14} /> Duplicate
        </button>
        <button
          class="mi danger"
          onclick={() => {
            void deleteTicket(ticket.id);
            close();
          }}
        >
          <Trash2 size={14} /> Delete
        </button>
      {:else if view === 'move'}
        <button class="mi back" onclick={() => (view = 'root')}>
          <ChevronLeft size={14} /> Move to
        </button>
        <div class="mdiv"></div>
        {#each lists as l (l.list)}
          <button
            class="mi"
            disabled={l.list === currentList}
            onclick={() => {
              void moveTicket(ticket.id, l.list, l.tickets.length);
              close();
            }}
          >
            <span class="mi-name">{l.name}</span>
            {#if l.list === currentList}<Check size={14} class="mi-check" />{/if}
          </button>
        {/each}
      {:else if view === 'priority'}
        <button class="mi back" onclick={() => (view = 'root')}>
          <ChevronLeft size={14} /> Priority
        </button>
        <div class="mdiv"></div>
        <button
          class="mi"
          onclick={() => {
            void updateTicket(ticket.id, { priority: null });
            close();
          }}
        >
          <Circle size={13} /> None
          {#if !ticket.priority}<Check size={14} class="mi-check" />{/if}
        </button>
        {#each $definitions.priorities as p (p)}
          <button
            class="mi"
            onclick={() => {
              void updateTicket(ticket.id, { priority: p });
              close();
            }}
          >
            <span class="dot" style="--d:{priorityColor(p)}"></span>
            <span class="mi-name">{p}</span>
            {#if ticket.priority === p}<Check size={14} class="mi-check" />{/if}
          </button>
        {/each}
      {:else if view === 'labels'}
        <button class="mi back" onclick={() => (view = 'root')}>
          <ChevronLeft size={14} /> Labels
        </button>
        <div class="mdiv"></div>
        {#if $definitions.labels.length === 0}
          <div class="mempty">No labels defined</div>
        {/if}
        {#each $definitions.labels as l (l.name)}
          <button class="mi" onclick={() => toggleLabel(l.name)}>
            <span class="dot" style="--d:{labelColorFor(l.name, $definitions.labels)}"></span>
            <span class="mi-name">{l.name}</span>
            {#if ticket.labels.includes(l.name)}<Check size={14} class="mi-check" />{/if}
          </button>
        {/each}
      {:else if view === 'assign'}
        <button class="mi back" onclick={() => (view = 'root')}>
          <ChevronLeft size={14} /> Assign
        </button>
        <div class="mdiv"></div>
        {#if $identities.length === 0}
          <div class="mempty">No identities</div>
        {/if}
        {#each $identities as id (id.id)}
          <button class="mi" onclick={() => toggleAssignee(id.id)}>
            <Avatar id={id.id} identity={id} size={18} />
            <span class="mi-name">{id.display_name || id.id}</span>
            {#if ticket.assignees.includes(id.id)}<Check size={14} class="mi-check" />{/if}
          </button>
        {/each}
      {/if}
    {/snippet}
  </Popover>
</div>

<style>
  .cm {
    display: inline-flex;
  }
  .cm-btn {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 24px;
    height: 24px;
    border-radius: var(--wp-r-sm);
    border: none;
    background: none;
    color: var(--wp-text-subtle);
    cursor: pointer;
    opacity: 0;
    transition:
      opacity var(--wp-fast) var(--wp-ease),
      background var(--wp-fast) var(--wp-ease),
      color var(--wp-fast) var(--wp-ease);
  }
  /* Revealed on card hover (see Card.svelte) or while the menu is open. */
  .cm-btn.open {
    opacity: 1;
    background: var(--wp-elevated);
    color: var(--wp-text);
  }
  .cm-btn:hover {
    background: var(--wp-elevated);
    color: var(--wp-text);
  }
  .mi {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
    min-width: 172px;
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
    opacity: 0.45;
    cursor: default;
  }
  .mi.danger {
    color: var(--wp-error);
  }
  .mi.back {
    color: var(--wp-text-muted);
    font-weight: 600;
  }
  .mi-name {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  :global(.mi .mi-chev) {
    margin-left: auto;
    color: var(--wp-text-subtle);
  }
  :global(.mi .mi-check) {
    margin-left: auto;
    color: var(--wp-accent);
    flex: none;
  }
  .dot {
    width: 9px;
    height: 9px;
    border-radius: 50%;
    background: var(--d);
    flex: none;
  }
  .mdiv {
    height: 1px;
    background: var(--wp-border);
    margin: 4px 0;
  }
  .mempty {
    padding: 8px;
    font-size: 12px;
    color: var(--wp-text-muted);
  }
</style>
