<script lang="ts">
  import { MessageSquare, Paperclip } from 'lucide-svelte';
  import Chip from './ui/Chip.svelte';
  import Avatar from './Avatar.svelte';
  import { definitions, identities } from '$lib/stores/board';
  import { labelColorFor, priorityColor } from '$lib/utils';
  import type { Ticket } from '$lib/types';

  let { ticket, onopen }: { ticket: Ticket; onopen: (t: Ticket) => void } = $props();

  let dot = $derived(priorityColor(ticket.priority));
  function identityFor(id: string) {
    return $identities.find((i) => i.id === id);
  }
</script>

<div
  class="card"
  role="button"
  tabindex="0"
  onclick={() => onopen(ticket)}
  onkeydown={(e) => (e.key === 'Enter' ? onopen(ticket) : null)}
>
  <div class="top">
    <span class="id">{ticket.id}</span>
    {#if ticket.priority}
      <span class="prio" style="--d:{dot}" title="Priority: {ticket.priority}"></span>
    {/if}
  </div>

  <div class="title">{ticket.title}</div>

  {#if ticket.labels.length}
    <div class="chips">
      {#each ticket.labels as label (label)}
        <Chip color={labelColorFor(label, $definitions.labels)}>{label}</Chip>
      {/each}
    </div>
  {/if}

  <div class="footer">
    <div class="avatars">
      {#each ticket.assignees.slice(0, 4) as a (a)}
        <Avatar id={a} identity={identityFor(a)} size={22} />
      {/each}
      {#if ticket.assignees.length > 4}
        <span class="more">+{ticket.assignees.length - 4}</span>
      {/if}
    </div>
    <div class="counts">
      {#if ticket.comments.length}
        <span class="count"><MessageSquare size={13} /> {ticket.comments.length}</span>
      {/if}
      {#if ticket.attachments.length}
        <span class="count"><Paperclip size={13} /> {ticket.attachments.length}</span>
      {/if}
    </div>
  </div>
</div>

<style>
  .card {
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 12px;
    background: var(--wp-card);
    border: 1px solid var(--wp-border);
    border-radius: var(--wp-r-md);
    cursor: pointer;
    transition:
      background var(--wp-fast) var(--wp-ease),
      border-color var(--wp-fast) var(--wp-ease),
      box-shadow var(--wp-fast) var(--wp-ease);
  }
  .card:hover {
    background: var(--wp-elevated);
    box-shadow: var(--wp-shadow);
  }
  .top {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }
  .id {
    font-family: var(--wp-font-mono);
    font-size: 12px;
    color: var(--wp-text-subtle);
  }
  .prio {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--d);
    flex: none;
  }
  .title {
    font-size: 14px;
    font-weight: 500;
    letter-spacing: -0.005em;
    line-height: 1.4;
    color: var(--wp-text);
    display: -webkit-box;
    -webkit-line-clamp: 3;
    line-clamp: 3;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }
  .chips {
    display: flex;
    flex-wrap: wrap;
    gap: 4px;
  }
  .footer {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    min-height: 22px;
  }
  .avatars {
    display: flex;
    align-items: center;
  }
  .avatars > :global(*:not(:first-child)) {
    margin-left: -6px;
  }
  .more {
    margin-left: 4px;
    font-size: 11px;
    color: var(--wp-text-subtle);
  }
  .counts {
    display: flex;
    gap: 10px;
    color: var(--wp-text-subtle);
  }
  .count {
    display: inline-flex;
    align-items: center;
    gap: 3px;
    font-size: 12px;
    font-family: var(--wp-font-mono);
  }
</style>
