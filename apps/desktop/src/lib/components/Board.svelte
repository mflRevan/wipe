<script lang="ts">
  import type { DndEvent } from 'svelte-dnd-action';
  import Column from './Column.svelte';
  import type { List, Ticket } from '$lib/types';
  import { api } from '$lib/api';
  import { board, currentProject, loadBoard, rewinding } from '$lib/stores/board';
  import { get } from 'svelte/store';

  interface Props {
    onopen?: (t: Ticket) => void;
    onadd?: (listId: string, listName: string) => void;
  }

  let { onopen, onadd }: Props = $props();

  // Local, mutable copy the dnd zones operate on. Re-synced whenever the
  // upstream board changes (WS refetch, project switch, rewind).
  let columns = $state<List[]>([]);

  let signature = $derived(
    $board
      ? $board.lists.map((l) => `${l.list}:${l.tickets.map((t) => t.id).join(',')}`).join('|')
      : ''
  );

  // Deep-clone board lists into local state on every meaningful change.
  let lastSig = '';
  $effect(() => {
    const sig = signature;
    if (sig === lastSig) return;
    lastSig = sig;
    columns = $board ? $board.lists.map((l) => ({ ...l, tickets: [...l.tickets] })) : [];
  });

  function setColumn(listId: string, items: Ticket[]) {
    columns = columns.map((c) => (c.list === listId ? { ...c, tickets: items } : c));
  }

  function handleConsider(listId: string, e: CustomEvent<DndEvent<Ticket>>) {
    setColumn(listId, e.detail.items);
  }

  async function handleFinalize(listId: string, e: CustomEvent<DndEvent<Ticket>>) {
    const items = e.detail.items;
    setColumn(listId, items);

    // Only the destination zone drives the move API call.
    if (e.detail.info.trigger !== 'droppedIntoZone') return;
    const movedId = e.detail.info.id;
    const pos = items.findIndex((t) => t.id === movedId);
    if (pos < 0) return;

    try {
      await api.moveTicket(movedId, listId, pos, get(currentProject) ?? undefined);
      // WS will refetch; also reload to reconcile in case WS is unavailable.
      await loadBoard();
    } catch {
      // Revert optimistic change on failure.
      await loadBoard();
    }
  }
</script>

<div class="scrollbar-thin flex h-full gap-4 overflow-x-auto pb-4">
  {#each columns as col (col.list)}
    <Column
      listId={col.list}
      name={col.name}
      tickets={col.tickets}
      dragDisabled={$rewinding}
      onconsider={handleConsider}
      onfinalize={handleFinalize}
      {onopen}
      onadd={(id) => onadd?.(id, col.name)}
    />
  {/each}

  {#if columns.length === 0}
    <div class="flex w-full items-center justify-center text-sm text-muted-foreground">
      This board has no lists.
    </div>
  {/if}
</div>
