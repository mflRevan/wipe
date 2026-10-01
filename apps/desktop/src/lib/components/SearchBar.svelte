<script lang="ts">
  // Board search: Ctrl/Cmd+F opens it (instead of the browser's find-in-page),
  // typing filters the board live, Escape or the close button ends it. It stays
  // open while tickets are opened and closed, so you can work through the hits.
  import { fly } from 'svelte/transition';
  import { Search, X } from 'lucide-svelte';
  import { searchQuery, searchMatches } from '$lib/stores/search';
  import { allTickets } from '$lib/stores/board';

  let { enabled = true }: { enabled?: boolean } = $props();

  let input = $state<HTMLInputElement | null>(null);
  let count = $derived($searchMatches?.size ?? 0);

  function open() {
    if ($searchQuery === null) searchQuery.set('');
    // Focus (and select, so a second Ctrl+F replaces the query) once rendered.
    requestAnimationFrame(() => {
      input?.focus();
      input?.select();
    });
  }
  function close() {
    searchQuery.set(null);
    input?.blur();
  }
  /** An open dialog (ticket, new card, lightbox, settings) owns Escape. */
  function dialogOpen() {
    return !!document.querySelector('[aria-modal="true"]');
  }

  function onKey(e: KeyboardEvent) {
    if (!enabled) return;
    if ((e.ctrlKey || e.metaKey) && !e.altKey && e.key.toLowerCase() === 'f') {
      e.preventDefault();
      open();
      return;
    }
    if (e.key === 'Escape' && $searchQuery !== null && !e.defaultPrevented) {
      // Escape inside the field always closes; elsewhere only when no dialog
      // is on top (that Escape belongs to the dialog).
      if (document.activeElement === input || !dialogOpen()) {
        e.preventDefault();
        close();
      }
    }
  }
</script>

<svelte:window onkeydown={onKey} />

{#if $searchQuery !== null && enabled}
  <div class="bar" role="search" transition:fly={{ y: -8, duration: 140 }}>
    <Search size={16} class="ico" />
    <input
      bind:this={input}
      type="text"
      placeholder="Search titles, descriptions, comments, labels, people, files…"
      spellcheck="false"
      autocomplete="off"
      value={$searchQuery}
      oninput={(e) => searchQuery.set(e.currentTarget.value)}
    />
    {#if ($searchQuery ?? '').trim()}
      <span class="count" class:none={count === 0}>{count} / {$allTickets.length}</span>
    {/if}
    <button class="close" onclick={close} title="Close search (Esc)" aria-label="Close search">
      <kbd>esc</kbd>
      <X size={15} />
    </button>
  </div>
{/if}

<style>
  .bar {
    align-self: center;
    flex: none;
    display: flex;
    align-items: center;
    gap: 10px;
    width: min(620px, calc(100% - 24px));
    height: 46px;
    padding: 0 8px 0 14px;
    border-radius: var(--wp-r-lg);
    border: 1px solid color-mix(in srgb, var(--wp-accent) 55%, var(--wp-border));
    background: var(--wp-card);
    box-shadow: var(--wp-shadow-lift);
  }
  .bar :global(.ico) {
    color: var(--wp-accent);
    flex: none;
  }
  input {
    flex: 1;
    min-width: 0;
    height: 100%;
    border: none;
    background: none;
    color: var(--wp-text);
    font-size: 15px;
    outline: none;
  }
  .bar input:focus,
  .bar input:focus-visible {
    outline: none;
    box-shadow: none;
  }
  .bar:focus-within {
    border-color: var(--wp-accent);
  }
  .count {
    flex: none;
    font-family: var(--wp-font-mono);
    font-size: 12px;
    color: var(--wp-text-muted);
  }
  .count.none {
    color: var(--wp-error);
  }
  .close {
    flex: none;
    display: inline-flex;
    align-items: center;
    gap: 6px;
    height: 30px;
    padding: 0 8px;
    border-radius: var(--wp-r-sm);
    border: 1px solid var(--wp-border);
    background: var(--wp-surface);
    color: var(--wp-text-muted);
    cursor: pointer;
  }
  .close:hover {
    color: var(--wp-text);
    background: var(--wp-elevated);
  }
  kbd {
    font-family: var(--wp-font-mono);
    font-size: 10px;
    letter-spacing: 0.04em;
    text-transform: uppercase;
  }
  @media (max-width: 700px) {
    kbd {
      display: none;
    }
  }
</style>
