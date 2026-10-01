<script lang="ts">
  // In-app notifications for watched items, shown while the board is in front
  // (native system notifications are used when the tab is in the background).
  import { fly } from 'svelte/transition';
  import { flip } from 'svelte/animate';
  import { X } from 'lucide-svelte';
  import { toasts, dismissToast, openRequest } from '$lib/stores/notify';
</script>

<div class="stack" aria-live="polite">
  {#each $toasts as t (t.id)}
    <div class="toast k-{t.kind}" animate:flip={{ duration: 160 }} transition:fly={{ x: 24, duration: 180 }}>
      <button
        class="body"
        onclick={() => {
          openRequest.set(t.target);
          dismissToast(t.id);
        }}
      >
        <span class="title">{t.title}</span>
        <span class="text">{t.body}</span>
      </button>
      <button class="x" aria-label="Dismiss" onclick={() => dismissToast(t.id)}><X size={13} /></button>
    </div>
  {/each}
</div>

<style>
  .stack {
    position: fixed;
    top: 64px;
    right: 16px;
    z-index: 120;
    display: flex;
    flex-direction: column;
    gap: 8px;
    width: min(360px, calc(100vw - 32px));
    pointer-events: none;
  }
  .toast {
    pointer-events: auto;
    display: flex;
    align-items: flex-start;
    border-radius: var(--wp-r-md);
    border: 1px solid var(--wp-border);
    border-left: 3px solid var(--wp-accent);
    background: var(--wp-card);
    box-shadow: var(--wp-shadow-lift);
  }
  .toast.k-comment,
  .toast.k-reply {
    border-left-color: #61aaf2;
  }
  .toast.k-done {
    border-left-color: #7e9b7a;
  }
  .toast.k-created {
    border-left-color: #e0a33b;
  }
  .body {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 3px;
    padding: 10px 4px 10px 12px;
    border: none;
    background: none;
    text-align: left;
    color: var(--wp-text);
    cursor: pointer;
  }
  .title {
    font-size: 13px;
    font-weight: 600;
  }
  .text {
    font-size: 12px;
    color: var(--wp-text-muted);
    white-space: pre-line;
    overflow: hidden;
    display: -webkit-box;
    -webkit-line-clamp: 3;
    line-clamp: 3;
    -webkit-box-orient: vertical;
  }
  .x {
    margin: 6px;
    padding: 4px;
    display: inline-flex;
    border: none;
    background: none;
    color: var(--wp-text-subtle);
    border-radius: var(--wp-r-sm);
    cursor: pointer;
  }
  .x:hover {
    color: var(--wp-text);
    background: var(--wp-elevated);
  }
</style>
