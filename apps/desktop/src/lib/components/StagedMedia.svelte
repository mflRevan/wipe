<script lang="ts">
  // In-place previews for media staged in a form but not saved yet: pasted or
  // picked files (shown from local object URLs) and pasted local paths (shown via
  // the daemon's this-machine preview). Tap any preview to inspect it full-screen.
  import { onDestroy } from 'svelte';
  import { X, FileText, File as FileIcon, Music, Film } from 'lucide-svelte';
  import { localFileUrl } from '$lib/api';
  import { openLightbox } from '$lib/stores/lightbox';
  import { formatBytes, mediaKind, type MediaKind } from '$lib/utils';

  let {
    files = [],
    paths = [],
    onremovefile,
    onremovepath
  }: {
    files?: File[];
    paths?: string[];
    onremovefile?: (i: number) => void;
    onremovepath?: (i: number) => void;
  } = $props();

  // One object URL per File, created lazily and revoked once the file is gone.
  const urls = new Map<File, string>();
  function urlFor(f: File): string {
    let u = urls.get(f);
    if (!u) {
      u = URL.createObjectURL(f);
      urls.set(f, u);
    }
    return u;
  }
  $effect(() => {
    for (const [f, u] of urls)
      if (!files.includes(f)) {
        URL.revokeObjectURL(u);
        urls.delete(f);
      }
  });
  onDestroy(() => {
    for (const u of urls.values()) URL.revokeObjectURL(u);
    urls.clear();
  });

  function baseName(p: string): string {
    return p.split(/[\\/]/).filter(Boolean).pop() ?? p;
  }

  type Entry = { key: string; name: string; meta: string; src: string; kind: MediaKind; remove: () => void };
  let entries = $derived<Entry[]>([
    ...files.map((f, i) => ({
      key: `f${i}:${f.name}:${f.size}`,
      name: f.name || 'pasted image',
      meta: formatBytes(f.size),
      src: urlFor(f),
      kind: mediaKind(f.type, f.name),
      remove: () => onremovefile?.(i)
    })),
    ...paths.map((p, i) => ({
      key: `p${i}:${p}`,
      name: baseName(p),
      meta: 'local path',
      src: localFileUrl(p),
      kind: mediaKind('', p),
      remove: () => onremovepath?.(i)
    }))
  ]);
  // Read-only strips (already-attached pastes) show no remove button.
  let removable = $derived(!!(onremovefile || onremovepath));
  // Paths the daemon couldn't preview (other machine, unsupported type) fall back
  // to a plain file tile.
  let broken = $state<Record<string, boolean>>({});
</script>

{#if entries.length}
  <div class="grid">
    {#each entries as e (e.key)}
      <div class="tile">
        <button
          class="preview"
          title={`Preview ${e.name}`}
          onclick={() => openLightbox({ src: e.src, name: e.name, kind: e.kind })}
        >
          {#if e.kind === 'image' && !broken[e.key]}
            <img src={e.src} alt={e.name} onerror={() => (broken = { ...broken, [e.key]: true })} />
          {:else if e.kind === 'video' && !broken[e.key]}
            <!-- svelte-ignore a11y_media_has_caption -->
            <video src={e.src} muted preload="metadata" onerror={() => (broken = { ...broken, [e.key]: true })}
            ></video>
            <span class="badge"><Film size={12} /></span>
          {:else}
            <span class="icon">
              {#if e.kind === 'audio'}<Music size={22} />{:else if e.kind === 'text' || e.kind === 'pdf'}<FileText
                  size={22}
                />{:else}<FileIcon size={22} />{/if}
            </span>
          {/if}
        </button>
        <div class="cap">
          <span class="name" title={e.name}>{e.name}</span>
          <span class="meta">{e.meta}</span>
        </div>
        {#if removable}
          <button class="rm" aria-label={`Remove ${e.name}`} title="Remove" onclick={e.remove}
            ><X size={13} /></button
          >
        {/if}
      </div>
    {/each}
  </div>
{/if}

<style>
  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(112px, 1fr));
    gap: 8px;
  }
  .tile {
    position: relative;
    display: flex;
    flex-direction: column;
    border: 1px solid var(--wp-border);
    border-radius: var(--wp-r-md);
    background: var(--wp-surface);
    overflow: hidden;
  }
  .preview {
    position: relative;
    display: flex;
    align-items: center;
    justify-content: center;
    height: 88px;
    padding: 0;
    border: none;
    background: var(--wp-canvas);
    cursor: zoom-in;
  }
  .preview img,
  .preview video {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }
  .badge {
    position: absolute;
    left: 6px;
    bottom: 6px;
    display: inline-flex;
    padding: 3px;
    border-radius: var(--wp-r-sm);
    background: rgba(0, 0, 0, 0.6);
    color: #fff;
  }
  .icon {
    color: var(--wp-text-muted);
  }
  .cap {
    display: flex;
    flex-direction: column;
    gap: 1px;
    padding: 5px 7px 6px;
    min-width: 0;
  }
  .name {
    font-size: 12px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .meta {
    font-family: var(--wp-font-mono);
    font-size: 10px;
    color: var(--wp-text-subtle);
  }
  .rm {
    position: absolute;
    top: 5px;
    right: 5px;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 24px;
    height: 24px;
    border-radius: 50%;
    border: none;
    background: rgba(0, 0, 0, 0.6);
    color: #fff;
    cursor: pointer;
  }
</style>
