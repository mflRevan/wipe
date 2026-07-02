<script lang="ts">
  import type { Snippet } from 'svelte';

  let {
    color,
    mono = false,
    onremove,
    onclick,
    children
  }: {
    color?: string;
    mono?: boolean;
    onremove?: () => void;
    onclick?: () => void;
    children: Snippet;
  } = $props();
</script>

<!-- svelte-ignore a11y_no_noninteractive_tabindex -->
<span
  class="chip"
  class:colored={!!color}
  class:mono
  class:clickable={!!onclick}
  style={color ? `--c:${color}` : ''}
  role={onclick ? 'button' : undefined}
  tabindex={onclick ? 0 : undefined}
  onclick={onclick}
  onkeydown={onclick ? (e) => (e.key === 'Enter' || e.key === ' ') && onclick() : undefined}
>
  {@render children()}
  {#if onremove}
    <button
      class="x"
      aria-label="Remove"
      onclick={(e) => {
        e.stopPropagation();
        onremove?.();
      }}>×</button
    >
  {/if}
</span>

<style>
  .chip {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    height: 20px;
    padding: 0 8px;
    border-radius: var(--wp-r-sm);
    font-size: 11px;
    font-weight: 500;
    line-height: 1;
    border: 1px solid var(--wp-border);
    background: var(--wp-surface);
    color: var(--wp-text-muted);
    white-space: nowrap;
  }
  .mono {
    font-family: var(--wp-font-mono);
  }
  .clickable {
    cursor: pointer;
  }
  /* Colored label chips: light theme = 14% bg / darkened text / 30% border. */
  .colored {
    background: color-mix(in srgb, var(--c) 14%, transparent);
    color: color-mix(in srgb, var(--c) 82%, black);
    border-color: color-mix(in srgb, var(--c) 30%, transparent);
  }
  :global([data-theme='dark']) .colored {
    background: color-mix(in srgb, var(--c) 22%, transparent);
    color: color-mix(in srgb, var(--c) 68%, white);
    border-color: color-mix(in srgb, var(--c) 40%, transparent);
  }
  .x {
    background: none;
    border: none;
    cursor: pointer;
    color: inherit;
    opacity: 0.6;
    font-size: 14px;
    line-height: 1;
    padding: 0;
    margin-right: -2px;
  }
  .x:hover {
    opacity: 1;
  }
</style>
