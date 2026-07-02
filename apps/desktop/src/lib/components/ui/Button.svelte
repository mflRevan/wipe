<script lang="ts">
  import type { Snippet } from 'svelte';

  let {
    variant = 'secondary',
    size = 'md',
    type = 'button',
    disabled = false,
    title,
    ariaLabel,
    onclick,
    children
  }: {
    variant?: 'primary' | 'secondary' | 'ghost' | 'danger';
    size?: 'sm' | 'md' | 'icon';
    type?: 'button' | 'submit';
    disabled?: boolean;
    title?: string;
    ariaLabel?: string;
    onclick?: (e: MouseEvent) => void;
    children: Snippet;
  } = $props();
</script>

<button
  {type}
  {disabled}
  {title}
  aria-label={ariaLabel}
  class="btn {variant} {size}"
  onclick={onclick}
>
  {@render children()}
</button>

<style>
  .btn {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 6px;
    border-radius: var(--wp-r-sm);
    border: 1px solid transparent;
    font-family: var(--wp-font-sans);
    font-weight: 500;
    cursor: pointer;
    white-space: nowrap;
    transition:
      background var(--wp-fast) var(--wp-ease),
      border-color var(--wp-fast) var(--wp-ease),
      color var(--wp-fast) var(--wp-ease);
  }
  .btn:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }
  .md {
    height: 32px;
    padding: 0 12px;
    font-size: 13px;
  }
  .sm {
    height: 26px;
    padding: 0 10px;
    font-size: 12px;
  }
  .icon {
    height: 32px;
    width: 32px;
    padding: 0;
  }
  .primary {
    background: var(--wp-accent);
    color: var(--wp-on-accent);
  }
  .primary:hover:not(:disabled) {
    background: var(--wp-accent-hover);
  }
  .secondary {
    background: var(--wp-card);
    border-color: var(--wp-border);
    color: var(--wp-text);
  }
  .secondary:hover:not(:disabled) {
    background: var(--wp-elevated);
    border-color: var(--wp-border-strong);
  }
  .ghost {
    background: transparent;
    color: var(--wp-text-muted);
  }
  .ghost:hover:not(:disabled) {
    background: var(--wp-elevated);
    color: var(--wp-text);
  }
  .danger {
    background: transparent;
    border-color: var(--wp-border);
    color: var(--wp-error);
  }
  .danger:hover:not(:disabled) {
    background: color-mix(in srgb, var(--wp-error) 12%, transparent);
    border-color: var(--wp-error);
  }
</style>
