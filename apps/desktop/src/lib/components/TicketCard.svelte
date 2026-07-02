<script lang="ts">
  import type { Ticket } from '$lib/types';
  import Badge from './ui/Badge.svelte';
  import { chipColor, priorityColor } from '$lib/utils';
  import { MessageSquare } from 'lucide-svelte';

  interface Props {
    ticket: Ticket;
    onopen?: (t: Ticket) => void;
    dragging?: boolean;
  }

  let { ticket, onopen, dragging = false }: Props = $props();
</script>

<button
  type="button"
  onclick={() => onopen?.(ticket)}
  class="group w-full cursor-pointer rounded-lg border border-border bg-card p-3 text-left shadow-sm transition-all hover:border-primary/40 hover:shadow-md focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
  class:opacity-60={dragging}
>
  <div class="flex items-center justify-between gap-2">
    <span class="font-mono text-[11px] text-muted-foreground">{ticket.id}</span>
    {#if ticket.priority}
      <Badge class={priorityColor(ticket.priority)}>{ticket.priority}</Badge>
    {/if}
  </div>

  <p class="mt-1.5 text-sm font-medium leading-snug text-foreground">{ticket.title}</p>

  {#if ticket.type || ticket.labels?.length}
    <div class="mt-2.5 flex flex-wrap gap-1.5">
      {#if ticket.type}
        <Badge class="border-primary/25 bg-primary/10 text-primary">{ticket.type}</Badge>
      {/if}
      {#each ticket.labels ?? [] as label (label)}
        <Badge class={chipColor(label)}>{label}</Badge>
      {/each}
    </div>
  {/if}

  {#if ticket.comments?.length || ticket.assignees?.length}
    <div class="mt-2.5 flex items-center gap-3 text-[11px] text-muted-foreground">
      {#if ticket.comments?.length}
        <span class="inline-flex items-center gap-1">
          <MessageSquare class="h-3 w-3" />
          {ticket.comments.length}
        </span>
      {/if}
      {#if ticket.assignees?.length}
        <span class="truncate">{ticket.assignees.join(', ')}</span>
      {/if}
    </div>
  {/if}
</button>
