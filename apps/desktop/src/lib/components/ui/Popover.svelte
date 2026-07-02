<script lang="ts">
  import type { Snippet } from 'svelte';

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

  function toggle() {
    open = !open;
  }
  function close() {
    open = false;
  }

  function onWindowClick(e: MouseEvent) {
    if (open && root && !root.contains(e.target as Node)) open = false;
  }
  function onKey(e: KeyboardEvent) {
    if (e.key === 'Escape') open = false;
  }
</script>

<svelte:window onclick={onWindowClick} onkeydown={onKey} />

<div class="pop-root" bind:this={root}>
  {@render trigger({ toggle, open })}
  {#if open}
    <div class="pop-panel wp-scroll {align}" style={width ? `width:${width}` : ''} role="menu">
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
    position: absolute;
    top: calc(100% + 6px);
    z-index: 60;
    min-width: 180px;
    max-height: 320px;
    overflow-y: auto;
    padding: 6px;
    background: var(--wp-card);
    border: 1px solid var(--wp-border);
    border-radius: var(--wp-r-lg);
    box-shadow: var(--wp-shadow-lift);
    animation: pop var(--wp-fast) var(--wp-ease);
  }
  .start {
    left: 0;
  }
  .end {
    right: 0;
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
