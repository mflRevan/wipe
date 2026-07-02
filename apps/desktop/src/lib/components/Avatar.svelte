<script lang="ts">
  import { Bot } from 'lucide-svelte';
  import { avatarColor, initials } from '$lib/utils';
  import type { Identity } from '$lib/types';

  let {
    id,
    identity,
    size = 22
  }: { id: string; identity?: Identity; size?: number } = $props();

  let name = $derived(identity?.display_name || id);
  let isAgent = $derived(identity?.kind === 'agent');
  let color = $derived(avatarColor(id));
</script>

<span
  class="avatar"
  style="--c:{color}; width:{size}px; height:{size}px; font-size:{Math.round(size * 0.42)}px"
  title={identity ? `${name} (${id})` : id}
>
  {initials(name)}
  {#if isAgent}
    <span class="bot" style="width:{Math.round(size * 0.5)}px; height:{Math.round(size * 0.5)}px">
      <Bot size={Math.round(size * 0.34)} />
    </span>
  {/if}
</span>

<style>
  .avatar {
    position: relative;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    border-radius: var(--wp-r-pill);
    background: color-mix(in srgb, var(--c) 30%, var(--wp-card));
    color: color-mix(in srgb, var(--c) 80%, var(--wp-text));
    border: 1px solid color-mix(in srgb, var(--c) 45%, transparent);
    font-family: var(--wp-font-sans);
    font-weight: 600;
    flex: none;
    user-select: none;
  }
  .bot {
    position: absolute;
    right: -3px;
    bottom: -3px;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    background: var(--wp-accent);
    color: var(--wp-on-accent);
    border: 1.5px solid var(--wp-card);
    border-radius: var(--wp-r-pill);
  }
</style>
