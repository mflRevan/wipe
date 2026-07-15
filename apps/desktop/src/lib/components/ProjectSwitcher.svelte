<script lang="ts">
  import { ChevronsUpDown, Check, Folder } from 'lucide-svelte';
  import Popover from './ui/Popover.svelte';
  import { projects, currentProject } from '$lib/stores/board';

  let { onselect }: { onselect: (path: string) => void } = $props();

  let current = $derived($projects.find((p) => p.path === $currentProject));

  /**
   * Turn a raw (possibly `\\?\`-verbatim) filesystem path into a compact,
   * readable label: the drive/root, an ellipsis if elided, then the project and
   * up to two ancestor directories - e.g.
   *   \\?\C:\Users\aimma\Workspace\ammar  ->  C:\...\aimma\Workspace\ammar
   */
  function compactPath(raw: string): string {
    const stripped = raw.replace(/^\\\\\?\\/, '').replace(/[\\/]+$/, '');
    const sep = stripped.includes('\\') ? '\\' : '/';
    const parts = stripped.split(/[\\/]+/).filter(Boolean);
    if (parts.length <= 1) return stripped;
    const driveLike = /^[A-Za-z]:$/.test(parts[0]);
    const root = driveLike ? parts[0] : sep === '/' ? '' : parts[0];
    const rest = driveLike ? parts.slice(1) : sep === '/' ? parts : parts.slice(1);
    const tail = rest.slice(-3); // project + up to two ancestors
    const elided = rest.length > tail.length;
    const segs: string[] = [];
    if (root) segs.push(root);
    if (elided) segs.push('…');
    segs.push(...tail);
    const joined = segs.join(sep);
    // Preserve a leading slash for unix absolute paths.
    return sep === '/' && !driveLike && stripped.startsWith('/') ? sep + joined : joined;
  }
</script>

<Popover>
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
          title={compactPath(p.path)}
        >
          <span class="pmeta">
            <span class="pname">{p.name}</span>
            <span class="ppath">{compactPath(p.path)}</span>
          </span>
          {#if p.path === $currentProject}<Check size={15} class="pcheck" />{/if}
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
    gap: 10px;
    width: 100%;
    /* Grow to fit the widest entry, but cap so a deep path can't stretch the
       menu off-screen; the path then truncates with an ellipsis. */
    max-width: 560px;
    padding: 7px 10px;
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
  /* Name and path on a single row (compact) - path sits to the right, muted. */
  .pmeta {
    display: flex;
    align-items: baseline;
    gap: 10px;
    min-width: 0;
    flex: 1;
  }
  .pname {
    font-size: 13px;
    font-weight: 500;
    flex: none;
    white-space: nowrap;
  }
  .ppath {
    font-family: var(--wp-font-mono);
    font-size: 11px;
    color: var(--wp-text-subtle);
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  :global(.ps-trigger .pcheck),
  .item :global(.pcheck) {
    flex: none;
    color: var(--wp-accent);
  }
  .empty {
    padding: 12px;
    font-size: 13px;
    color: var(--wp-text-muted);
    text-align: center;
  }
</style>
