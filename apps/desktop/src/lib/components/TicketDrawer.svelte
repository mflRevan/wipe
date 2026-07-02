<script lang="ts">
  import { api } from '$lib/api';
  import { board, currentProject, loadBoard, rewinding } from '$lib/stores/board';
  import type { Ticket } from '$lib/types';
  import { get } from 'svelte/store';
  import Badge from './ui/Badge.svelte';
  import Button from './ui/Button.svelte';
  import Textarea from './ui/Textarea.svelte';
  import { chipColor, formatDate, priorityColor } from '$lib/utils';
  import { X } from 'lucide-svelte';

  interface Props {
    ticketId?: string | null;
  }

  let { ticketId = $bindable(null) }: Props = $props();

  // Resolve the live ticket from the board store so comments stay fresh.
  let ticket = $derived.by<Ticket | null>(() => {
    if (!ticketId || !$board) return null;
    for (const l of $board.lists) {
      const t = l.tickets.find((x) => x.id === ticketId);
      if (t) return t;
    }
    return null;
  });

  let listName = $derived.by<string>(() => {
    if (!ticketId || !$board) return '';
    for (const l of $board.lists) {
      if (l.tickets.some((x) => x.id === ticketId)) return l.name;
    }
    return '';
  });

  let comment = $state('');
  let submitting = $state(false);
  let error = $state<string | null>(null);

  function close() {
    ticketId = null;
  }

  function onkeydown(e: KeyboardEvent) {
    if (e.key === 'Escape') close();
  }

  async function addComment() {
    if (!ticket || !comment.trim() || submitting) return;
    submitting = true;
    error = null;
    try {
      await api.addComment(ticket.id, comment.trim(), undefined, get(currentProject) ?? undefined);
      comment = '';
      await loadBoard();
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    } finally {
      submitting = false;
    }
  }
</script>

<svelte:window onkeydown={ticketId ? onkeydown : undefined} />

{#if ticketId}
  <div class="fixed inset-0 z-40 flex justify-end">
    <button class="absolute inset-0 bg-black/50" aria-label="Close" onclick={close}></button>

    <aside
      class="animate-slide relative z-10 flex h-full w-full max-w-md flex-col border-l border-border bg-card shadow-2xl"
    >
      {#if ticket}
        <header class="flex items-start justify-between gap-3 border-b border-border p-5">
          <div class="min-w-0">
            <div class="flex items-center gap-2">
              <span class="font-mono text-xs text-muted-foreground">{ticket.id}</span>
              {#if listName}
                <span class="text-xs text-muted-foreground">· {listName}</span>
              {/if}
            </div>
            <h2 class="mt-1 text-lg font-semibold leading-snug">{ticket.title}</h2>
          </div>
          <button
            class="rounded-md p-1 text-muted-foreground transition-colors hover:bg-accent hover:text-foreground"
            aria-label="Close"
            onclick={close}
          >
            <X class="h-4 w-4" />
          </button>
        </header>

        <div class="scrollbar-thin flex-1 space-y-6 overflow-y-auto p-5">
          <!-- metadata -->
          <div class="flex flex-wrap gap-1.5">
            {#if ticket.type}
              <Badge class="border-primary/25 bg-primary/10 text-primary">{ticket.type}</Badge>
            {/if}
            {#if ticket.priority}
              <Badge class={priorityColor(ticket.priority)}>{ticket.priority}</Badge>
            {/if}
            {#each ticket.labels ?? [] as label (label)}
              <Badge class={chipColor(label)}>{label}</Badge>
            {/each}
            {#each ticket.tags ?? [] as tag (tag)}
              <Badge class="border-border bg-muted text-muted-foreground">#{tag}</Badge>
            {/each}
          </div>

          {#if ticket.assignees?.length}
            <div class="text-sm">
              <span class="text-muted-foreground">Assignees:</span>
              {ticket.assignees.join(', ')}
            </div>
          {/if}

          {#if ticket.body}
            <div>
              <h3 class="mb-1.5 text-xs font-medium uppercase tracking-wide text-muted-foreground">
                Description
              </h3>
              <p class="whitespace-pre-wrap text-sm leading-relaxed text-foreground/90">
                {ticket.body}
              </p>
            </div>
          {/if}

          <div class="text-xs text-muted-foreground">
            Created {formatDate(ticket.created)} · Updated {formatDate(ticket.updated)}
          </div>

          <!-- comments -->
          <div>
            <h3 class="mb-2 text-xs font-medium uppercase tracking-wide text-muted-foreground">
              Comments ({ticket.comments?.length ?? 0})
            </h3>
            <div class="space-y-3">
              {#each ticket.comments ?? [] as c (c.id)}
                <div class="rounded-lg border border-border bg-background/50 p-3">
                  <div class="mb-1 flex items-center justify-between text-xs">
                    <span class="font-medium text-foreground/80">{c.author}</span>
                    <span class="text-muted-foreground">{formatDate(c.created)}</span>
                  </div>
                  <p class="whitespace-pre-wrap text-sm text-foreground/90">{c.body}</p>
                </div>
              {/each}
              {#if !ticket.comments?.length}
                <p class="text-sm text-muted-foreground">No comments yet.</p>
              {/if}
            </div>
          </div>
        </div>

        {#if !$rewinding}
          <footer class="border-t border-border p-4">
            <form
              class="space-y-2"
              onsubmit={(e) => {
                e.preventDefault();
                void addComment();
              }}
            >
              <Textarea
                bind:value={comment}
                placeholder="Add a comment…"
                class="min-h-[60px]"
              />
              {#if error}
                <p class="text-xs text-destructive">{error}</p>
              {/if}
              <div class="flex justify-end">
                <Button type="submit" size="sm" disabled={!comment.trim() || submitting}>
                  {submitting ? 'Posting…' : 'Comment'}
                </Button>
              </div>
            </form>
          </footer>
        {:else}
          <footer class="border-t border-border p-4 text-center text-xs text-muted-foreground">
            Read-only historical snapshot
          </footer>
        {/if}
      {:else}
        <div class="flex h-full items-center justify-center p-6 text-sm text-muted-foreground">
          Ticket not found.
        </div>
      {/if}
    </aside>
  </div>
{/if}

<style>
  .animate-slide {
    animation: slide-in 0.18s ease-out both;
  }
  @keyframes slide-in {
    from {
      transform: translateX(100%);
    }
    to {
      transform: translateX(0);
    }
  }
</style>
