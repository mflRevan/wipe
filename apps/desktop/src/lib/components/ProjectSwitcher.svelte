<script lang="ts">
  import { ChevronsUpDown, Check, Folder } from 'lucide-svelte';
  import Popover from './ui/Popover.svelte';
  import { projects, currentProject } from '$lib/stores/board';

  let { onselect }: { onselect: (path: string) => void } = $props();

  let current = $derived($projects.find((p) => p.path === $currentProject));
</script>

<Popover width="280px">
  {#snippet trigger({ toggle })}
    <button class="ps-trigger" onclick={toggle} title="Switch project">
      <Folder size={14} />
      <span class="name">{current?.name ?? 'No project'}</span>
      <ChevronsUpDown size={14} class="chev" />
    </button>
  {/snippet}
  {#snippet children({ close })}
    {#if $projects.length === 0}
      <div class="empty">No projects registered</div>
    {:else}
      {#each $projects as p (p.path)}
        <button
          class="item"
          class:active={p.path === $currentProject}
          onclick={() => {
            onselect(p.path);
            close();
          }}
        >
          <span class="col">
            <span class="pname">{p.name}</span>
            <span class="ppath">{p.path}</span>
          </span>
          {#if p.path === $currentProject}<Check size={15} />{/if}
        </button>
      {/each}
    {/if}
  {/snippet}
</Popover>

<style>
  .ps-trigger {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    height: 32px;
    padding: 0 10px;
    border-radius: var(--wp-r-sm);
    border: 1px solid var(--wp-border);
    background: var(--wp-card);
    color: var(--wp-text);
    cursor: pointer;
    max-width: 260px;
    transition: all var(--wp-fast) var(--wp-ease);
  }
  .ps-trigger:hover {
    background: var(--wp-elevated);
    border-color: var(--wp-border-strong);
  }
  .name {
    font-size: 13px;
    font-weight: 500;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  :global(.ps-trigger .chev) {
    color: var(--wp-text-subtle);
    flex: none;
  }
  .item {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    width: 100%;
    padding: 8px 10px;
    border: none;
    background: none;
    color: var(--wp-text);
    border-radius: var(--wp-r-sm);
    cursor: pointer;
    text-align: left;
  }
  .item:hover {
    background: var(--wp-elevated);
  }
  .item.active {
    color: var(--wp-accent);
  }
  .col {
    display: flex;
    flex-direction: column;
    gap: 1px;
    overflow: hidden;
  }
  .pname {
    font-size: 13px;
    font-weight: 500;
  }
  .ppath {
    font-family: var(--wp-font-mono);
    font-size: 11px;
    color: var(--wp-text-subtle);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .empty {
    padding: 12px;
    font-size: 13px;
    color: var(--wp-text-muted);
    text-align: center;
  }
</style>
