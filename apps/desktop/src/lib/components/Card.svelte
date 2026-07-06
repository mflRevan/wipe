<script lang="ts">
  import { MessageSquare, Paperclip } from 'lucide-svelte';
  import Chip from './ui/Chip.svelte';
  import Avatar from './Avatar.svelte';
  import { definitions, identities, currentProject, recentlyChanged } from '$lib/stores/board';
  import { mediaUrl } from '$lib/api';
  import { labelColorFor, priorityColor, mediaKind } from '$lib/utils';
  import type { Attachment, Ticket } from '$lib/types';

  let { ticket, onopen }: { ticket: Ticket; onopen: (t: Ticket) => void } = $props();

  let dot = $derived(priorityColor(ticket.priority));
  // Briefly highlight when an agent/human changed this card since the last poll.
  let changed = $derived($recentlyChanged.has(ticket.id));
  // First image attachment becomes a compact card cover, like Trello.
  let cover = $derived<Attachment | undefined>(
    ticket.attachments.find((a) => mediaKind(a.mime, a.name) === 'image')
  );
  function identityFor(id: string) {
    return $identities.find((i) => i.id === id);
  }
</script>

<div
  class="card"
  class:changed
  role="button"
  tabindex="0"
  onclick={() => onopen(ticket)}
  onkeydown={(e) => (e.key === 'Enter' ? onopen(ticket) : null)}
>
  {#if cover}
    <div class="cover">
      <img
        src={mediaUrl(cover.path, $currentProject ?? undefined)}
        alt={cover.name}
        loading="lazy"
      />
    </div>
  {/if}

  {#if ticket.labels.length}
    <div class="chips">
      {#each ticket.labels as label (label)}
        <Chip color={labelColorFor(label, $definitions.labels)}>{label}</Chip>
      {/each}
    </div>
  {/if}

  <div class="title">
    {#if ticket.priority}
      <span class="prio" style="--d:{dot}" title="Priority: {ticket.priority}"></span>
    {/if}
    <span class="ttext">{ticket.title}</span>
  </div>

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
  /* Brief highlight when an agent/human just changed this card (live updates). */
  .card.changed {
    animation: card-flash 1.8s var(--wp-ease);
  }
  @keyframes card-flash {
    0% {
      border-color: var(--wp-accent);
      box-shadow: 0 0 0 3px color-mix(in srgb, var(--wp-accent) 30%, transparent);
    }
    100% {
      border-color: var(--wp-border);
      box-shadow: none;
    }
  }
  @media (prefers-reduced-motion: reduce) {
    .card.changed {
      animation: none;
      border-color: var(--wp-accent);
    }
  }
  .cover {
    margin: -12px -12px 0;
    max-height: 120px;
    overflow: hidden;
    border-radius: var(--wp-r-md) var(--wp-r-md) 0 0;
    background: var(--wp-surface);
  }
  .cover img {
    display: block;
    width: 100%;
    max-height: 120px;
    object-fit: cover;
  }
  .prio {
    width: 8px;
    height: 8px;
    margin-top: 5px;
    border-radius: 50%;
    background: var(--d);
    flex: none;
  }
  .title {
    display: flex;
    align-items: flex-start;
    gap: 6px;
    font-size: 14px;
    font-weight: 500;
    letter-spacing: -0.005em;
    line-height: 1.4;
    color: var(--wp-text);
  }
  .ttext {
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
