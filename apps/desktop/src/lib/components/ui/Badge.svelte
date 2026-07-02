<script lang="ts">
  import { cn } from '$lib/utils';
  import type { Snippet } from 'svelte';

  interface Props {
    class?: string;
    children?: Snippet;
  }

  let { class: className = '', children }: Props = $props();
</script>

<span
  class={cn(
    'inline-flex items-center rounded-full border px-2 py-0.5 text-[11px] font-medium leading-none',
    className
  )}
>
  {@render children?.()}
</span>
