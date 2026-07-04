<script lang="ts">
  import { Trash2, CornerDownRight } from 'lucide-svelte';
  import Self from './ForumPost.svelte';
  import Avatar from './Avatar.svelte';
  import Markdown from './Markdown.svelte';
  import Chip from './ui/Chip.svelte';
  import { identities, definitions } from '$lib/stores/board';
  import { formatDate, labelColorFor } from '$lib/utils';
  import type { ForumPost } from '$lib/types';

  let {
    post,
    depth = 0,
    onreply,
    ondelete
  }: {
    post: ForumPost;
    depth?: number;
    onreply: (id: string) => void;
    ondelete: (id: string) => void;
  } = $props();

  function identityFor(id: string) {
    const direct = $identities.find((i) => i.id === id);
    if (direct) return direct;
    const m = id.match(/<([^>]+)>/);
    return m ? $identities.find((i) => i.id === m[1]) : undefined;
  }
  function displayName(id: string) {
    const found = identityFor(id);
    if (found) return found.display_name;
    const m = id.match(/^(.*?)\s*<[^>]+>$/);
    return m && m[1] ? m[1] : id;
  }
  // Only show labels that still exist in the board's label pool; anything else is
  // stray data and should never render as a free-form tag.
  let poolLabels = $derived(
    (post.labels ?? []).filter((l) => $definitions.labels.some((d) => d.name === l))
  );
</script>

<div class="node">
  <div class="row">
    <Avatar id={identityFor(post.author)?.id ?? post.author} identity={identityFor(post.author)} size={26} />
    <div class="bubble">
      <div class="p-head">
        <span class="p-author">{displayName(post.author)}</span>
        <span class="p-time">{formatDate(post.created)}{post.edited ? ' · edited' : ''}</span>
        <div class="p-actions">
          <button class="pa" title="Reply" onclick={() => onreply(post.id)}>
            <CornerDownRight size={13} />
          </button>
          <button class="pa danger" title="Delete (and replies)" onclick={() => ondelete(post.id)}>
            <Trash2 size={13} />
          </button>
        </div>
      </div>
      {#if poolLabels.length}
        <div class="p-labels">
          {#each poolLabels as l (l)}
            <Chip color={labelColorFor(l, $definitions.labels)}>{l}</Chip>
          {/each}
        </div>
      {/if}
      {#if post.body?.trim()}
        <div class="p-body"><Markdown source={post.body} /></div>
      {/if}
    </div>
  </div>

  {#if post.replies?.length}
    <div class="children">
      {#each post.replies as reply (reply.id)}
        <Self post={reply} depth={depth + 1} {onreply} {ondelete} />
      {/each}
    </div>
  {/if}
</div>

<style>
  .node {
    display: flex;
    flex-direction: column;
    gap: 10px;
  }
  .row {
    display: flex;
    gap: 10px;
    align-items: flex-start;
  }
  .bubble {
    flex: 1;
    min-width: 0;
    border: 1px solid var(--wp-border);
    border-radius: var(--wp-r-md);
    background: var(--wp-surface);
    padding: 8px 11px;
  }
  .p-head {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .p-author {
    font-size: 13px;
    font-weight: 600;
  }
  .p-time {
    font-size: 11px;
    color: var(--wp-text-subtle);
  }
  .p-actions {
    margin-left: auto;
    display: flex;
    gap: 2px;
    opacity: 0;
    transition: opacity var(--wp-fast) var(--wp-ease);
  }
  .row:hover .p-actions {
    opacity: 1;
  }
  .pa {
    display: inline-flex;
    padding: 4px;
    border: none;
    background: none;
    color: var(--wp-text-subtle);
    cursor: pointer;
    border-radius: var(--wp-r-sm);
  }
  .pa:hover {
    color: var(--wp-text);
    background: var(--wp-elevated);
  }
  .pa.danger:hover {
    color: var(--wp-error);
  }
  .p-labels {
    display: flex;
    flex-wrap: wrap;
    gap: 4px;
    margin: 5px 0 2px;
  }
  .p-body {
    font-size: 13px;
    line-height: 1.5;
    margin-top: 3px;
  }
  /* A continuous rail down each nesting level draws the connector to the parent
     thread; the avatar column (26px) + gap keeps replies aligned under their op. */
  .children {
    display: flex;
    flex-direction: column;
    gap: 10px;
    margin-left: 13px;
    padding-left: 18px;
    border-left: 2px solid var(--wp-border);
  }
</style>
