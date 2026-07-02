<script lang="ts">
  import { dndzone, type DndEvent } from 'svelte-dnd-action';
  import type { Ticket } from '$lib/types';
  import TicketCard from './TicketCard.svelte';
  import Button from './ui/Button.svelte';
  import { Plus } from 'lucide-svelte';

  interface Props {
    listId: string;
    name: string;
    tickets: Ticket[];
    dragDisabled?: boolean;
    onconsider?: (listId: string, e: CustomEvent<DndEvent<Ticket>>) => void;
    onfinalize?: (listId: string, e: CustomEvent<DndEvent<Ticket>>) => void;
    onadd?: (listId: string) => void;
    onopen?: (t: Ticket) => void;
  }

  let {
    listId,
    name,
    tickets,
    dragDisabled = false,
    onconsider,
    onfinalize,
    onadd,
    onopen
  }: Props = $props();

  const flipDurationMs = 160;
</script>

<div class="flex h-full w-72 shrink-0 flex-col rounded-xl border border-border bg-muted/30">
  <div class="flex items-center justify-between px-3 pb-2 pt-3">
    <div class="flex items-center gap-2">
      <h3 class="text-sm font-semibold tracking-tight">{name}</h3>
      <span
        class="rounded-full bg-muted px-1.5 py-0.5 text-[11px] font-medium text-muted-foreground"
      >
        {tickets.length}
      </span>
    </div>
    {#if !dragDisabled}
      <Button
        variant="ghost"
        size="icon"
        class="h-7 w-7"
        aria-label={`Add ticket to ${name}`}
        onclick={() => onadd?.(listId)}
      >
        <Plus class="h-4 w-4" />
      </Button>
    {/if}
  </div>

  <section
    class="scrollbar-thin flex flex-1 flex-col gap-2 overflow-y-auto px-2 pb-2"
    use:dndzone={{ items: tickets, dragDisabled, flipDurationMs, dropTargetStyle: {} }}
    onconsider={(e) => onconsider?.(listId, e)}
    onfinalize={(e) => onfinalize?.(listId, e)}
  >
    {#each tickets as ticket (ticket.id)}
      <div>
        <TicketCard {ticket} {onopen} />
      </div>
    {/each}

    {#if tickets.length === 0}
      <div
        class="mx-1 mt-1 rounded-lg border border-dashed border-border/70 py-8 text-center text-xs text-muted-foreground"
      >
        No tickets
      </div>
    {/if}
  </section>
</div>
