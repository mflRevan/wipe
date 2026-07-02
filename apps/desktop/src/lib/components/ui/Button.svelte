<script lang="ts">
  import { cn } from '$lib/utils';
  import type { Snippet } from 'svelte';
  import type { HTMLButtonAttributes } from 'svelte/elements';

  type Variant = 'default' | 'secondary' | 'ghost' | 'outline' | 'destructive';
  type Size = 'sm' | 'md' | 'lg' | 'icon';

  interface Props extends HTMLButtonAttributes {
    variant?: Variant;
    size?: Size;
    class?: string;
    children?: Snippet;
  }

  let {
    variant = 'default',
    size = 'md',
    class: className = '',
    children,
    ...rest
  }: Props = $props();

  const variants: Record<Variant, string> = {
    default: 'bg-primary text-primary-foreground hover:bg-primary/90 shadow-sm',
    secondary: 'bg-secondary text-secondary-foreground hover:bg-secondary/80',
    ghost: 'hover:bg-accent hover:text-accent-foreground',
    outline: 'border border-border bg-transparent hover:bg-accent hover:text-accent-foreground',
    destructive: 'bg-destructive text-destructive-foreground hover:bg-destructive/90'
  };

  const sizes: Record<Size, string> = {
    sm: 'h-8 px-3 text-xs',
    md: 'h-9 px-4 text-sm',
    lg: 'h-10 px-6 text-sm',
    icon: 'h-9 w-9'
  };
</script>

<button
  class={cn(
    'inline-flex select-none items-center justify-center gap-2 rounded-md font-medium transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 focus-visible:ring-offset-background disabled:pointer-events-none disabled:opacity-50',
    variants[variant],
    sizes[size],
    className
  )}
  {...rest}
>
  {@render children?.()}
</button>
