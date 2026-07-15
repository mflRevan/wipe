<script lang="ts">
  import { onMount } from 'svelte';
  import { Trash2, RotateCcw, X } from 'lucide-svelte';
  import {
    trash,
    trashRetentionDays,
    loadTrash,
    restoreTicket,
    purgeTrashTicket,
    emptyTrash,
    setTrashRetention
  } from '$lib/stores/board';

  let {
    dragActive = false,
    overTrash = false,
    binEl = $bindable(null)
  }: {
    dragActive?: boolean;
    // Whether the dragged pointer is currently over the bin (Board hit-tests the
    // POINTER, not the card, and drives this - a release then deletes).
    overTrash?: boolean;
    // The bin element, exposed so Board can hit-test the pointer against it.
    binEl?: HTMLElement | null;
  } = $props();

  let over = $derived(dragActive && overTrash);

  let panelOpen = $state(false);
  let root = $state<HTMLDivElement>();
  let editingRetention = $state(false);
  let retentionDraft = $state('');

  // Load the trash once so the badge count is present before the panel is opened.
  onMount(() => {
    void loadTrash();
  });

  async function toggle() {
    if (dragActive) return;
    panelOpen = !panelOpen;
    if (panelOpen) await loadTrash();
  }

  function startEditRetention() {
    retentionDraft = String($trashRetentionDays);
    editingRetention = true;
  }
  async function commitRetention() {
    editingRetention = false;
    const n = parseInt(retentionDraft, 10);
    if (Number.isFinite(n) && n >= 0 && n !== $trashRetentionDays) {
      await setTrashRetention(n);
    }
  }

  /** Compact "time since deletion" label. */
  function since(iso: string): string {
    const then = new Date(iso).getTime();
    const s = Math.max(0, Math.floor((Date.now() - then) / 1000));
    if (s < 60) return 'just now';
    const m = Math.floor(s / 60);
    if (m < 60) return `${m}m ago`;
    const h = Math.floor(m / 60);
    if (h < 24) return `${h}h ago`;
    const d = Math.floor(h / 24);
    return `${d}d ago`;
  }

  function onWindowPointer(e: MouseEvent) {
    if (panelOpen && root && !e.composedPath().includes(root)) panelOpen = false;
  }
  function onKey(e: KeyboardEvent) {
    if (e.key === 'Escape' && panelOpen) {
      panelOpen = false;
      e.stopPropagation();
    }
  }
</script>

<svelte:window onpointerdown={onWindowPointer} onkeydown={onKey} />

<div
  class="trash-wrap"
  class:drag={dragActive}
  class:over
  class:open={panelOpen}
  bind:this={root}
>
  {#if panelOpen && !dragActive}
    <div class="trash-panel wp-scroll">
      <header class="tp-head">
        <span class="tp-title">Trash</span>
        {#if $trash.length > 0}
          <button class="tp-empty" onclick={() => emptyTrash()}>Empty</button>
        {/if}
      </header>

      {#if $trash.length === 0}
        <div class="tp-empty-state">No deleted tickets.</div>
      {:else}
        <ul class="tp-list">
          {#each $trash as t (t.id)}
            <li class="tp-item">
              <button
                class="tp-restore"
                title="Restore {t.id}"
                onclick={() => restoreTicket(t.id)}
              >
                <span class="tp-main">
                  <span class="tp-t-title">{t.title}</span>
                  <span class="tp-meta">{t.id} · deleted {since(t.deleted_at)}</span>
                </span>
                <span class="tp-overlay"><RotateCcw size={15} /> Restore</span>
              </button>
              <button
                class="tp-purge"
                title="Delete forever"
                onclick={() => purgeTrashTicket(t.id)}
              >
                <X size={14} />
              </button>
            </li>
          {/each}
        </ul>
      {/if}

      <footer class="tp-foot">
        {#if editingRetention}
          <!-- svelte-ignore a11y_autofocus -->
          <span class="tp-ret">
            Kept for
            <input
              class="tp-ret-input"
              type="number"
              min="0"
              autofocus
              bind:value={retentionDraft}
              onblur={commitRetention}
              onkeydown={(e) => {
                if (e.key === 'Enter') e.currentTarget.blur();
                else if (e.key === 'Escape') editingRetention = false;
              }}
            />
            days
          </span>
        {:else}
          <button class="tp-ret" onclick={startEditRetention} title="Change retention (global)">
            Kept for <strong>{$trashRetentionDays}</strong> day{$trashRetentionDays === 1 ? '' : 's'}
          </button>
        {/if}
      </footer>
    </div>
  {/if}

  <!-- The bin itself: always visible, small; grows into a drop target on drag,
       and is clickable (with a hover lift) to open the restore panel. -->
  <div
    class="trash-bin"
    bind:this={binEl}
    role="button"
    tabindex="0"
    aria-label={dragActive ? 'Drop to delete' : 'Open trash'}
    onclick={toggle}
    onkeydown={(e) => {
      if (e.key === 'Enter' || e.key === ' ') {
        e.preventDefault();
        void toggle();
      }
    }}
  >
    <Trash2 size={dragActive ? (over ? 24 : 20) : 17} />
    {#if dragActive}
      <span class="trash-hint">{over ? 'Release to delete' : 'Drop to delete'}</span>
    {:else if $trash.length > 0}
      <span class="trash-badge">{$trash.length}</span>
    {/if}
  </div>
</div>

<style>
  .trash-wrap {
    position: fixed;
    right: 22px;
    bottom: 22px;
    z-index: 60;
    display: flex;
    flex-direction: column;
    align-items: flex-end;
    gap: 10px;
  }

  /* Base bin: small, muted, always present. */
  .trash-bin {
    position: relative;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 6px;
    width: 42px;
    height: 42px;
    border-radius: var(--wp-r-lg);
    border: 1.5px solid var(--wp-border);
    background: var(--wp-card);
    color: var(--wp-text-subtle);
    cursor: pointer;
    box-shadow: var(--wp-shadow-sm, 0 2px 6px -2px rgb(0 0 0 / 0.15));
    transition:
      width var(--wp-base) var(--wp-ease),
      height var(--wp-base) var(--wp-ease),
      transform var(--wp-fast) var(--wp-ease),
      background var(--wp-fast) var(--wp-ease),
      border-color var(--wp-fast) var(--wp-ease),
      color var(--wp-fast) var(--wp-ease),
      box-shadow var(--wp-fast) var(--wp-ease);
  }
  /* Hover (no drag): gentle lift + accent, so it reads as clickable. */
  .trash-wrap:not(.drag) .trash-bin:hover {
    color: var(--wp-text);
    border-color: var(--wp-border-strong);
    background: var(--wp-elevated);
    transform: translateY(-2px);
  }
  .trash-wrap.open .trash-bin {
    color: var(--wp-text);
    border-color: var(--wp-border-strong);
    background: var(--wp-elevated);
  }

  /* Dragging a card: the bin expands into a big, red drop target. */
  .trash-wrap.drag .trash-bin {
    width: 120px;
    height: 120px;
    border-style: dashed;
    border-color: color-mix(in srgb, var(--wp-error) 45%, transparent);
    background: color-mix(in srgb, var(--wp-error) 8%, var(--wp-surface));
    color: var(--wp-error);
  }
  .trash-wrap.over .trash-bin {
    width: 140px;
    height: 140px;
    border-style: solid;
    border-color: var(--wp-error);
    background: color-mix(in srgb, var(--wp-error) 16%, var(--wp-surface));
    box-shadow:
      0 12px 30px -10px color-mix(in srgb, var(--wp-error) 55%, transparent),
      0 0 0 4px color-mix(in srgb, var(--wp-error) 22%, transparent);
    transform: scale(1.03);
  }

  .trash-hint {
    font-family: var(--wp-font-display);
    font-size: 10px;
    font-weight: 600;
    letter-spacing: 0.05em;
    text-transform: uppercase;
    line-height: 1.2;
    max-width: 96px;
    text-align: center;
  }
  .trash-badge {
    position: absolute;
    top: -6px;
    right: -6px;
    min-width: 18px;
    height: 18px;
    padding: 0 5px;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    border-radius: var(--wp-r-pill);
    background: var(--wp-accent);
    color: var(--wp-on-accent, #fff);
    font-family: var(--wp-font-mono);
    font-size: 10px;
    font-weight: 600;
  }

  /* --- restore panel ----------------------------------------------------- */
  .trash-panel {
    width: 320px;
    max-height: min(60vh, 460px);
    overflow-y: auto;
    padding: 6px;
    background: var(--wp-card);
    border: 1px solid var(--wp-border);
    border-radius: var(--wp-r-lg);
    box-shadow: var(--wp-shadow-lift);
    animation: tp-in var(--wp-fast) var(--wp-ease);
  }
  @keyframes tp-in {
    from {
      opacity: 0;
      transform: translateY(6px) scale(0.98);
    }
    to {
      opacity: 1;
      transform: translateY(0) scale(1);
    }
  }
  .tp-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 6px 8px 8px;
  }
  .tp-title {
    font-family: var(--wp-font-display);
    font-size: 11px;
    font-weight: 700;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--wp-text-muted);
  }
  .tp-empty {
    border: none;
    background: none;
    color: var(--wp-text-subtle);
    font-size: 12px;
    cursor: pointer;
    padding: 2px 6px;
    border-radius: var(--wp-r-sm);
  }
  .tp-empty:hover {
    color: var(--wp-error);
    background: color-mix(in srgb, var(--wp-error) 10%, transparent);
  }
  .tp-empty-state {
    padding: 20px 12px;
    text-align: center;
    color: var(--wp-text-muted);
    font-size: 13px;
  }
  .tp-list {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .tp-item {
    position: relative;
    display: flex;
    align-items: stretch;
  }
  .tp-restore {
    position: relative;
    flex: 1;
    min-width: 0;
    display: flex;
    align-items: center;
    padding: 8px 10px;
    border: none;
    background: none;
    color: var(--wp-text);
    border-radius: var(--wp-r-sm);
    cursor: pointer;
    text-align: left;
    overflow: hidden;
  }
  .tp-restore:hover {
    background: var(--wp-elevated);
  }
  .tp-main {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
  }
  .tp-t-title {
    font-size: 13px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .tp-meta {
    font-family: var(--wp-font-mono);
    font-size: 11px;
    color: var(--wp-text-subtle);
  }
  /* Hover-fade "Restore" overlay across the row. */
  .tp-overlay {
    position: absolute;
    inset: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 6px;
    background: color-mix(in srgb, var(--wp-accent) 90%, transparent);
    color: var(--wp-on-accent, #fff);
    font-size: 12px;
    font-weight: 600;
    letter-spacing: 0.02em;
    opacity: 0;
    transition: opacity var(--wp-fast) var(--wp-ease);
    border-radius: var(--wp-r-sm);
  }
  .tp-restore:hover .tp-overlay {
    opacity: 1;
  }
  .tp-purge {
    flex: none;
    width: 30px;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    border: none;
    background: none;
    color: var(--wp-text-subtle);
    cursor: pointer;
    border-radius: var(--wp-r-sm);
  }
  .tp-purge:hover {
    color: var(--wp-error);
    background: color-mix(in srgb, var(--wp-error) 10%, transparent);
  }
  .tp-foot {
    display: flex;
    justify-content: flex-end;
    padding: 8px 8px 4px;
    margin-top: 4px;
    border-top: 1px solid var(--wp-border);
  }
  .tp-ret {
    border: none;
    background: none;
    color: var(--wp-text-subtle);
    font-size: 11px;
    cursor: pointer;
    padding: 2px 4px;
    border-radius: var(--wp-r-sm);
  }
  .tp-ret:hover {
    color: var(--wp-text);
  }
  .tp-ret strong {
    color: var(--wp-text);
  }
  .tp-ret-input {
    width: 44px;
    margin: 0 4px;
    padding: 1px 4px;
    border: 1px solid var(--wp-border-strong);
    border-radius: var(--wp-r-sm);
    background: var(--wp-surface);
    color: var(--wp-text);
    font-size: 11px;
  }

  @media (prefers-reduced-motion: reduce) {
    .trash-bin,
    .trash-panel {
      transition: none;
      animation: none;
    }
  }
</style>
