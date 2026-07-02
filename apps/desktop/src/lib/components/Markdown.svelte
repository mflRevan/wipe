<script lang="ts">
  import { renderMarkdown } from '$lib/utils';
  let { source }: { source: string } = $props();
  let html = $derived(renderMarkdown(source));
</script>

<!-- eslint-disable-next-line svelte/no-at-html-tags -->
<div class="md">{@html html}</div>

<style>
  .md {
    font-size: 14px;
    line-height: 1.55;
    color: var(--wp-text);
    word-break: break-word;
  }
  .md :global(h1),
  .md :global(h2),
  .md :global(h3) {
    font-family: var(--wp-font-display);
    font-weight: 600;
    line-height: 1.25;
    margin: 0.8em 0 0.4em;
  }
  .md :global(h1) {
    font-size: 20px;
  }
  .md :global(h2) {
    font-size: 17px;
  }
  .md :global(h3) {
    font-size: 15px;
  }
  .md :global(p) {
    margin: 0 0 0.7em;
  }
  .md :global(ul),
  .md :global(ol) {
    margin: 0 0 0.7em;
    padding-left: 1.4em;
  }
  .md :global(li) {
    margin: 0.2em 0;
  }
  .md :global(a) {
    color: var(--wp-accent);
    text-decoration: underline;
    text-underline-offset: 2px;
  }
  .md :global(code) {
    font-family: var(--wp-font-mono);
    font-size: 12.5px;
    background: var(--wp-surface);
    border: 1px solid var(--wp-border);
    border-radius: var(--wp-r-sm);
    padding: 1px 5px;
  }
  .md :global(pre) {
    background: var(--wp-surface);
    border: 1px solid var(--wp-border);
    border-radius: var(--wp-r-md);
    padding: 12px;
    overflow-x: auto;
  }
  .md :global(pre code) {
    background: none;
    border: none;
    padding: 0;
  }
  .md :global(blockquote) {
    margin: 0 0 0.7em;
    padding-left: 12px;
    border-left: 3px solid var(--wp-border-strong);
    color: var(--wp-text-muted);
  }
  .md :global(table) {
    border-collapse: collapse;
    margin: 0 0 0.7em;
  }
  .md :global(th),
  .md :global(td) {
    border: 1px solid var(--wp-border);
    padding: 4px 8px;
    text-align: left;
  }
  .md :global(img) {
    max-width: 100%;
    border-radius: var(--wp-r-md);
  }
  .md :global(hr) {
    border: none;
    border-top: 1px solid var(--wp-border);
    margin: 1em 0;
  }
</style>
