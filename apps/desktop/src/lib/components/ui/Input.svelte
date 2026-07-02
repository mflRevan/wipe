<script lang="ts">
  import { cn } from '$lib/utils';
  import type { HTMLInputAttributes } from 'svelte/elements';

  interface Props extends HTMLInputAttributes {
    value?: string;
    class?: string;
  }

  let { value = $bindable(''), class: className = '', ...rest }: Props = $props();
</script>

<input
  bind:value
  class={cn(
    'flex h-9 w-full rounded-md border border-input bg-background px-3 py-1 text-sm shadow-sm transition-colors placeholder:text-muted-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring disabled:cursor-not-allowed disabled:opacity-50',
    className
  )}
  {...rest}
/>
