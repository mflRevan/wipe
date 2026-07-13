<script lang="ts">
  import { dndzone, type DndEvent } from 'svelte-dnd-action';
  import { Trash2 } from 'lucide-svelte';
  import type { Ticket } from '$lib/types';

  let {
    dragActive = false,
    flipMs = 150,
    ondelete
  }: {
    dragActive?: boolean;
    flipMs?: number;
    ondelete: (id: string) => void;
  } = $props();

  // svelte-dnd-action drop zone. The bin holds nothing of its own; while a card is
  // dragged over it, the library places a shadow item here, which we detect (to
  // glow) but never render as a card. On drop, whatever landed is deleted.
  let items = $state<Ticket[]>([]);
  // A card is currently hovering over the bin (drop would delete it).
  let over = $derived(items.length > 0);

  function consider(e: CustomEvent<DndEvent<Ticket>>) {
    items = e.detail.items;
  }
  function finalize(e: CustomEvent<DndEvent<Ticket>>) {
    const dropped = e.detail.items;
    items = [];
    for (const t of dropped) ondelete(t.id);
  }
</script>

<!-- Always mounted so the drop zone is registered before a drag begins; only
     visible/interactive while a ticket is being dragged. -->
<div class="trash-wrap" class:active={dragActive} class:over aria-hidden={!dragActive}>
  <div
    class="trash-zone"
    use:dndzone={{
      items,
      flipDurationMs: flipMs,
      dropTargetStyle: {},
      // The bin never shows a dragged card - just a hidden placeholder that keeps
      // the zone valid while hovering.
      transformDraggedElement: (el?: HTMLElement) => {
        if (el) el.style.opacity = '0';
      }
    }}
    onconsider={consider}
    onfinalize={finalize}
  >
    {#each items as t (t.id)}
      <div class="sink" aria-hidden="true"></div>
    {/each}
  </div>
  <div class="trash-face">
    <Trash2 size={over ? 26 : 20} />
    <span class="trash-label">{over ? 'Release to delete' : 'Drop to delete'}</span>
  </div>
</div>

<style>
  .trash-wrap {
    position: fixed;
    right: 22px;
    bottom: 22px;
    z-index: 60;
    display: flex;
    align-items: center;
    justify-content: center;
    /* Collapsed + non-interactive until a drag starts. */
    width: 60px;
    height: 60px;
    padding: 0;
    border-radius: var(--wp-r-lg);
    border: 1.5px dashed transparent;
    background: transparent;
    color: var(--wp-text-subtle);
    opacity: 0;
    transform: translateY(12px) scale(0.9);
    pointer-events: none;
    transition:
      opacity var(--wp-base) var(--wp-ease),
      transform var(--wp-base) var(--wp-ease),
      width var(--wp-base) var(--wp-ease),
      height var(--wp-base) var(--wp-ease),
      background var(--wp-fast) var(--wp-ease),
      border-color var(--wp-fast) var(--wp-ease),
      color var(--wp-fast) var(--wp-ease),
      box-shadow var(--wp-fast) var(--wp-ease);
  }
  .trash-wrap.active {
    opacity: 1;
    transform: translateY(0) scale(1);
    width: 128px;
    height: 128px;
    pointer-events: auto;
    border-color: color-mix(in srgb, var(--wp-error) 45%, transparent);
    background: color-mix(in srgb, var(--wp-error) 7%, var(--wp-surface));
    color: var(--wp-error);
  }
  /* A card is hovering: enlarge + glow, making the delete target unmissable. */
  .trash-wrap.over {
    width: 148px;
    height: 148px;
    border-style: solid;
    border-color: var(--wp-error);
    background: color-mix(in srgb, var(--wp-error) 16%, var(--wp-surface));
    box-shadow:
      0 12px 30px -10px color-mix(in srgb, var(--wp-error) 55%, transparent),
      0 0 0 4px color-mix(in srgb, var(--wp-error) 22%, transparent);
    transform: translateY(0) scale(1.04);
  }
  /* The dnd zone fills the bin so a card is easy to drop anywhere on it. */
  .trash-zone {
    position: absolute;
    inset: 0;
    border-radius: inherit;
  }
  .sink {
    width: 100%;
    height: 100%;
  }
  .trash-face {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 6px;
    pointer-events: none;
    text-align: center;
  }
  .trash-label {
    font-family: var(--wp-font-display);
    font-size: 10px;
    font-weight: 600;
    letter-spacing: 0.05em;
    text-transform: uppercase;
    line-height: 1.2;
    max-width: 88px;
  }
  .trash-wrap:not(.active) .trash-label {
    display: none;
  }
  @media (prefers-reduced-motion: reduce) {
    .trash-wrap {
      transition: opacity var(--wp-fast) var(--wp-ease);
      transform: none;
    }
    .trash-wrap.active,
    .trash-wrap.over {
      transform: none;
    }
  }
</style>
