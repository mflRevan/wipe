<script lang="ts">
  import { cn } from '$lib/utils';
  import { X } from 'lucide-svelte';
  import type { Snippet } from 'svelte';

  interface Props {
    open?: boolean;
    title?: string;
    description?: string;
    class?: string;
    onclose?: () => void;
    children?: Snippet;
    footer?: Snippet;
  }

  let {
    open = $bindable(false),
    title = '',
    description = '',
    class: className = '',
    onclose,
    children,
    footer
  }: Props = $props();

  function close() {
    open = false;
    onclose?.();
  }

  function onkeydown(e: KeyboardEvent) {
    if (e.key === 'Escape') close();
  }
</script>

<svelte:window onkeydown={open ? onkeydown : undefined} />

{#if open}
  <div class="fixed inset-0 z-50 flex items-center justify-center p-4">
    <!-- backdrop -->
    <button
      class="absolute inset-0 bg-black/60 backdrop-blur-sm"
      aria-label="Close dialog"
      onclick={close}
    ></button>

    <div
      role="dialog"
      aria-modal="true"
      class={cn(
        'relative z-10 w-full max-w-lg rounded-xl border border-border bg-card p-6 shadow-2xl',
        'animate-in',
        className
      )}
    >
      <div class="mb-4 flex items-start justify-between gap-4">
        <div>
          {#if title}
            <h2 class="text-lg font-semibold tracking-tight">{title}</h2>
          {/if}
          {#if description}
            <p class="mt-1 text-sm text-muted-foreground">{description}</p>
          {/if}
        </div>
        <button
          class="rounded-md p-1 text-muted-foreground transition-colors hover:bg-accent hover:text-foreground"
          aria-label="Close"
          onclick={close}
        >
          <X class="h-4 w-4" />
        </button>
      </div>

      <div>{@render children?.()}</div>

      {#if footer}
        <div class="mt-6 flex justify-end gap-2">{@render footer()}</div>
      {/if}
    </div>
  </div>
{/if}

<style>
  .animate-in {
    animation: dialog-in 0.15s ease-out both;
  }
  @keyframes dialog-in {
    from {
      opacity: 0;
      transform: translateY(8px) scale(0.98);
    }
    to {
      opacity: 1;
      transform: translateY(0) scale(1);
    }
  }
</style>
