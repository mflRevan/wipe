<script lang="ts">
  import type { Snippet } from 'svelte';
  import { tick } from 'svelte';

  let {
    open = $bindable(false),
    align = 'start',
    width,
    trigger,
    children
  }: {
    open?: boolean;
    align?: 'start' | 'end';
    width?: string;
    trigger: Snippet<[{ toggle: () => void; open: boolean }]>;
    children: Snippet<[{ close: () => void }]>;
  } = $props();

  let root = $state<HTMLDivElement>();
  let panel = $state<HTMLDivElement>();
  let style = $state('position:fixed; visibility:hidden;');

  function toggle() {
    open = !open;
  }
  function close() {
    open = false;
  }

  // Position the panel with viewport-fixed coordinates anchored to the trigger.
  // Using `fixed` keeps it out of the trigger's layout flow so it can never be
  // clipped by a scrolling/overflow ancestor (e.g. the ticket modal) nor pushed
  // off-screen when the trigger sits at the end of a wrapping row of chips.
  function place() {
    if (!open || !root) return;
    const r = root.getBoundingClientRect();
    const gap = 6;
    const margin = 8;
    const vw = window.innerWidth;
    const vh = window.innerHeight;
    const pw = panel?.offsetWidth ?? (width ? parseInt(width) : 200);
    const ph = panel?.offsetHeight ?? 0;

    let left = align === 'end' ? r.right - pw : r.left;
    left = Math.min(Math.max(margin, left), Math.max(margin, vw - pw - margin));

    // Prefer opening below; flip above only if it overflows and there's room.
    let top = r.bottom + gap;
    if (ph && top + ph > vh - margin && r.top - gap - ph > margin) {
      top = r.top - gap - ph;
    }
    top = Math.max(margin, Math.min(top, Math.max(margin, vh - margin - ph)));

    style = `position:fixed; top:${Math.round(top)}px; left:${Math.round(left)}px; ${
      width ? `width:${width};` : ''
    } visibility:visible;`;
  }

  $effect(() => {
    if (!open) return;
    // Render hidden, measure, then place - avoids a one-frame flash at (0,0).
    style = `${width ? `width:${width};` : ''} position:fixed; visibility:hidden;`;
    tick().then(place);
    const reposition = () => place();
    // Capture-phase catches scrolling inside nested scroll containers too.
    window.addEventListener('scroll', reposition, true);
    window.addEventListener('resize', reposition);
    return () => {
      window.removeEventListener('scroll', reposition, true);
      window.removeEventListener('resize', reposition);
    };
  });

  // `composedPath()` is captured at dispatch, so a click on a control that
  // removes itself mid-handler (e.g. "New label" swapping to a form) is still
  // recognized as inside the popover and does not close it.
  function onWindowPointer(e: MouseEvent) {
    if (open && root && !e.composedPath().includes(root)) open = false;
  }
  function onKey(e: KeyboardEvent) {
    if (e.key === 'Escape') open = false;
  }
</script>

<svelte:window onpointerdown={onWindowPointer} onkeydown={onKey} />

<div class="pop-root" bind:this={root}>
  {@render trigger({ toggle, open })}
  {#if open}
    <div class="pop-panel wp-scroll" bind:this={panel} {style} role="menu">
      {@render children({ close })}
    </div>
  {/if}
</div>

<style>
  .pop-root {
    position: relative;
    display: inline-flex;
  }
  .pop-panel {
    position: fixed;
    z-index: 200;
    min-width: 180px;
    max-height: min(360px, 80vh);
    overflow-y: auto;
    padding: 6px;
    background: var(--wp-card);
    border: 1px solid var(--wp-border);
    border-radius: var(--wp-r-lg);
    box-shadow: var(--wp-shadow-lift);
    animation: pop var(--wp-fast) var(--wp-ease);
  }
  @keyframes pop {
    from {
      opacity: 0;
      transform: translateY(-4px);
    }
    to {
      opacity: 1;
      transform: translateY(0);
    }
  }
</style>
