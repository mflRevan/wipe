<script lang="ts">
  // Ticket quick-jump: with nothing focused, typing "T" and then the id's digits
  // (the dash is optional - "T2a", "t-02A", "T5") shows the query large in the
  // middle of the screen, highlights the matching card and scrolls it into view.
  // Enter opens it; Escape or any click dismisses.
  import { fade, scale } from 'svelte/transition';
  import { tick } from 'svelte';
  import { board } from '$lib/stores/board';
  import { jumpQuery, jumpTarget, resolveJump, searchMatches } from '$lib/stores/search';
  import type { Ticket } from '$lib/types';

  let { enabled = true, onopen }: { enabled?: boolean; onopen: (t: Ticket) => void } = $props();

  let target = $derived(resolveJump($board, $jumpQuery ?? ''));
  let hex = $derived($board?.ids === 'hex');
  let hidden = $derived(!!target && !!$searchMatches && !$searchMatches.has(target.id));

  /** Typing into a field, or a dialog/menu on top: not ours. */
  function busy(e: KeyboardEvent) {
    const el = document.activeElement as HTMLElement | null;
    const editable =
      !!el &&
      (el.tagName === 'INPUT' ||
        el.tagName === 'TEXTAREA' ||
        el.tagName === 'SELECT' ||
        el.isContentEditable);
    return (
      editable ||
      e.ctrlKey ||
      e.metaKey ||
      e.altKey ||
      !!document.querySelector('[aria-modal="true"]')
    );
  }

  function stop() {
    jumpQuery.set(null);
    jumpTarget.set(null);
  }

  async function reveal(t: Ticket | null) {
    jumpTarget.set(t?.id ?? null);
    if (!t) return;
    await tick();
    const el = document.querySelector(`[data-ticket-id="${CSS.escape(t.id)}"]`);
    el?.scrollIntoView({ behavior: 'smooth', block: 'nearest', inline: 'nearest' });
  }

  function onKey(e: KeyboardEvent) {
    if (!enabled) return;
    const q = $jumpQuery;
    if (q === null) {
      if ((e.key === 't' || e.key === 'T') && !busy(e)) {
        e.preventDefault();
        jumpQuery.set('');
        jumpTarget.set(null);
      }
      return;
    }
    // A jump is in progress: it owns the keyboard until it ends.
    if (e.key === 'Escape') {
      e.preventDefault();
      stop();
    } else if (e.key === 'Enter') {
      e.preventDefault();
      const t = target;
      stop();
      if (t) onopen(t);
    } else if (e.key === 'Backspace') {
      e.preventDefault();
      if (q.length === 0) stop();
      else jumpQuery.set(q.slice(0, -1));
    } else if (e.key === '-' || e.key === ' ') {
      e.preventDefault(); // the dash (and a space) are optional separators
    } else if (/^[0-9a-fA-F]$/.test(e.key) && q.length < 8) {
      e.preventDefault();
      jumpQuery.set(q + e.key.toUpperCase());
    }
  }

  $effect(() => {
    if ($jumpQuery !== null) void reveal(target);
  });
</script>

<svelte:window onkeydown={onKey} onpointerdown={() => $jumpQuery !== null && stop()} />

{#if $jumpQuery !== null}
  <div class="jump" transition:fade={{ duration: 100 }} aria-live="polite">
    <div class="pill" transition:scale={{ duration: 140, start: 0.92 }}>
      <span class="q">T-{$jumpQuery}<span class="caret"></span></span>
      {#if !$jumpQuery}
        <span class="hint">type the ticket number{hex ? ' (e.g. 2A)' : ''}</span>
      {:else if target}
        <span class="hit"><b>{target.id}</b> {target.title}</span>
        <span class="hint"
          >{hidden ? 'hidden by the current search - ' : ''}Enter to open · Esc to close</span
        >
      {:else}
        <span class="miss">no ticket T-{$jumpQuery}</span>
      {/if}
    </div>
  </div>
{/if}

<style>
  .jump {
    position: fixed;
    inset: 0;
    z-index: 90;
    display: flex;
    align-items: center;
    justify-content: center;
    pointer-events: none;
  }
  .pill {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 8px;
    max-width: min(560px, calc(100vw - 32px));
    padding: 22px 30px;
    border-radius: var(--wp-r-lg);
    border: 1px solid color-mix(in srgb, var(--wp-accent) 50%, var(--wp-border));
    background: color-mix(in srgb, var(--wp-card) 92%, transparent);
    backdrop-filter: blur(8px);
    box-shadow: var(--wp-shadow-lift);
  }
  .q {
    font-family: var(--wp-font-mono);
    font-size: 44px;
    font-weight: 600;
    letter-spacing: 0.04em;
    color: var(--wp-text);
  }
  .caret {
    display: inline-block;
    width: 3px;
    height: 0.9em;
    margin-left: 3px;
    transform: translateY(6px);
    background: var(--wp-accent);
    animation: blink 0.8s steps(1) infinite;
  }
  @keyframes blink {
    50% {
      opacity: 0;
    }
  }
  .hit {
    max-width: 100%;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    color: var(--wp-text);
    font-size: 15px;
  }
  .hit b {
    font-family: var(--wp-font-mono);
    color: var(--wp-accent);
    margin-right: 6px;
  }
  .hint {
    font-size: 12px;
    color: var(--wp-text-subtle);
  }
  .miss {
    font-size: 14px;
    color: var(--wp-error);
  }
</style>
