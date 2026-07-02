<script lang="ts">
  import { Upload, Download, Trash2, FileText, File as FileIcon } from 'lucide-svelte';
  import { get } from 'svelte/store';
  import { api, mediaUrl } from '$lib/api';
  import { currentProject } from '$lib/stores/board';
  import { formatBytes, mediaKind } from '$lib/utils';
  import type { Attachment } from '$lib/types';

  let {
    ticketId,
    attachments,
    readOnly = false
  }: { ticketId: string; attachments: Attachment[]; readOnly?: boolean } = $props();

  let uploading = $state(false);
  let error = $state<string | null>(null);
  let dragOver = $state(false);
  let fileInput = $state<HTMLInputElement>();

  function url(a: Attachment): string {
    return mediaUrl(a.path, get(currentProject) ?? undefined);
  }

  async function uploadFiles(files: FileList | File[]) {
    error = null;
    uploading = true;
    try {
      for (const f of Array.from(files)) {
        await api.uploadAttachment(ticketId, f, get(currentProject) ?? undefined);
      }
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    } finally {
      uploading = false;
    }
  }

  async function remove(a: Attachment) {
    error = null;
    try {
      await api.deleteAttachment(ticketId, a.path, get(currentProject) ?? undefined);
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    }
  }

  function onDrop(e: DragEvent) {
    e.preventDefault();
    dragOver = false;
    if (readOnly) return;
    if (e.dataTransfer?.files?.length) void uploadFiles(e.dataTransfer.files);
  }
</script>

<div
  class="att"
  class:drag={dragOver}
  role="region"
  aria-label="Attachments"
  ondragover={(e) => {
    if (!readOnly) {
      e.preventDefault();
      dragOver = true;
    }
  }}
  ondragleave={() => (dragOver = false)}
  ondrop={onDrop}
>
  {#each attachments as a (a.path)}
    {@const kind = mediaKind(a.mime, a.name)}
    <div class="entry">
      <div class="entry-head">
        <span class="fname" title={a.path}>{a.name}</span>
        <span class="fmeta">{formatBytes(a.size)} · {a.source}</span>
        <a class="iconbtn" href={url(a)} download={a.name} title="Download"><Download size={14} /></a>
        {#if !readOnly}
          <button class="iconbtn danger" title="Delete" onclick={() => remove(a)}>
            <Trash2 size={14} />
          </button>
        {/if}
      </div>

      {#if kind === 'image'}
        <a href={url(a)} target="_blank" rel="noreferrer">
          <img class="media" src={url(a)} alt={a.name} loading="lazy" />
        </a>
      {:else if kind === 'audio'}
        <!-- svelte-ignore a11y_media_has_caption -->
        <audio controls src={url(a)}></audio>
      {:else if kind === 'video'}
        <!-- svelte-ignore a11y_media_has_caption -->
        <video class="media" controls src={url(a)}></video>
      {:else if kind === 'text' || kind === 'pdf'}
        <div class="filecard">
          <FileText size={18} />
          <span class="fc-name">{a.name}</span>
          <a class="dl" href={url(a)} download={a.name}>Download</a>
        </div>
      {:else}
        <div class="filecard">
          <FileIcon size={18} />
          <span class="fc-name">{a.name}</span>
          <a class="dl" href={url(a)} download={a.name}>Download</a>
        </div>
      {/if}
    </div>
  {/each}

  {#if !readOnly}
    <input
      type="file"
      multiple
      hidden
      bind:this={fileInput}
      onchange={(e) => {
        const t = e.target as HTMLInputElement;
        if (t.files?.length) void uploadFiles(t.files);
        t.value = '';
      }}
    />
    <button class="dropzone" onclick={() => fileInput?.click()} disabled={uploading}>
      <Upload size={15} />
      {uploading ? 'Uploading…' : 'Upload or drop files here'}
    </button>
  {/if}

  {#if error}<div class="err">{error}</div>{/if}
</div>

<style>
  .att {
    display: flex;
    flex-direction: column;
    gap: 10px;
    border-radius: var(--wp-r-md);
    transition: box-shadow var(--wp-fast) var(--wp-ease);
  }
  .att.drag {
    box-shadow: 0 0 0 2px var(--wp-accent);
  }
  .entry {
    border: 1px solid var(--wp-border);
    border-radius: var(--wp-r-md);
    padding: 8px;
    background: var(--wp-card);
  }
  .entry-head {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-bottom: 6px;
  }
  .fname {
    font-size: 13px;
    font-weight: 500;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .fmeta {
    font-family: var(--wp-font-mono);
    font-size: 11px;
    color: var(--wp-text-subtle);
    margin-left: auto;
    flex: none;
  }
  .iconbtn {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 26px;
    height: 26px;
    border-radius: var(--wp-r-sm);
    border: 1px solid var(--wp-border);
    background: var(--wp-card);
    color: var(--wp-text-muted);
    cursor: pointer;
    flex: none;
  }
  .iconbtn:hover {
    background: var(--wp-elevated);
    color: var(--wp-text);
  }
  .iconbtn.danger:hover {
    color: var(--wp-error);
    border-color: var(--wp-error);
  }
  .media {
    display: block;
    max-width: 100%;
    max-height: 320px;
    border-radius: var(--wp-r-sm);
  }
  audio {
    width: 100%;
  }
  .filecard {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 10px;
    border-radius: var(--wp-r-sm);
    background: var(--wp-surface);
    color: var(--wp-text-muted);
  }
  .fc-name {
    font-size: 13px;
    color: var(--wp-text);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .dl {
    margin-left: auto;
    font-size: 12px;
    color: var(--wp-accent);
    text-decoration: none;
    flex: none;
  }
  .dl:hover {
    text-decoration: underline;
  }
  .dropzone {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 8px;
    padding: 14px;
    border: 1px dashed var(--wp-border-strong);
    border-radius: var(--wp-r-md);
    background: none;
    color: var(--wp-text-muted);
    font-size: 13px;
    cursor: pointer;
    transition: all var(--wp-fast) var(--wp-ease);
  }
  .dropzone:hover:not(:disabled) {
    background: var(--wp-elevated);
    color: var(--wp-text);
  }
  .err {
    font-size: 12px;
    color: var(--wp-error);
  }
</style>
