<script lang="ts">
  import { fade, scale } from 'svelte/transition';
  import { browser } from '$app/environment';
  import { get } from 'svelte/store';
  import { X, Paperclip } from 'lucide-svelte';
  import Button from './ui/Button.svelte';
  import LabelPicker from './LabelPicker.svelte';
  import AssigneePicker from './AssigneePicker.svelte';
  import { api } from '$lib/api';
  import { definitions, currentProject } from '$lib/stores/board';
  import { formatBytes } from '$lib/utils';

  let {
    open = $bindable(false),
    listId,
    listName
  }: { open?: boolean; listId: string; listName: string } = $props();

  const reduced = browser && matchMedia('(prefers-reduced-motion: reduce)').matches;
  const dur = reduced ? 0 : 160;

  let title = $state('');
  let priority = $state('');
  let body = $state('');
  let labels = $state<string[]>([]);
  let assignees = $state<string[]>([]);
  let files = $state<File[]>([]);
  let error = $state<string | null>(null);
  let busy = $state(false);

  function reset() {
    title = '';
    priority = '';
    body = '';
    labels = [];
    assignees = [];
    files = [];
    error = null;
  }

  function onFiles(e: Event) {
    const input = e.currentTarget as HTMLInputElement;
    if (input.files) files = [...files, ...Array.from(input.files)];
    input.value = '';
  }
  function removeFile(i: number) {
    files = files.filter((_, idx) => idx !== i);
  }

  async function submit() {
    const t = title.trim();
    if (!t) return;
    busy = true;
    error = null;
    const project = get(currentProject) ?? undefined;
    try {
      const ticket = await api.createTicket(
        {
          title: t,
          list: listId,
          priority: priority || undefined,
          body: body || undefined,
          labels: labels.length ? labels : undefined,
          assignees: assignees.length ? assignees : undefined
        },
        project
      );
      // Attachments are per-ticket, so upload them once the ticket exists.
      for (const f of files) {
        await api.uploadAttachment(ticket.id, f, project);
      }
      reset();
      open = false;
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    } finally {
      busy = false;
    }
  }
</script>

{#if open}
  <div
    class="scrim"
    transition:fade={{ duration: dur }}
    onclick={() => (open = false)}
    role="presentation"
  ></div>
  <div class="modal-wrap">
    <div
      class="modal wp-scroll"
      transition:scale={{ duration: dur, start: 0.96 }}
      role="dialog"
      aria-modal="true"
    >
      <header class="m-head">
        <h3>New card in <span class="ln">{listName}</span></h3>
        <button class="close" aria-label="Close" onclick={() => (open = false)}
          ><X size={18} /></button
        >
      </header>

      <label class="fl" for="nt-title">Title</label>
      <!-- svelte-ignore a11y_autofocus -->
      <input
        id="nt-title"
        class="in"
        autofocus
        bind:value={title}
        onkeydown={(e) => e.key === 'Enter' && submit()}
        placeholder="What needs doing?"
      />

      <label class="fl" for="nt-prio">Priority</label>
      <select id="nt-prio" class="in" bind:value={priority}>
        <option value="">- none -</option>
        {#each $definitions.priorities as p (p)}<option value={p}>{p}</option>{/each}
      </select>

      <span class="fl">Labels</span>
      <LabelPicker selected={labels} onchange={(v) => (labels = v)} />

      <span class="fl">Members</span>
      <AssigneePicker selected={assignees} onchange={(v) => (assignees = v)} />

      <label class="fl" for="nt-body">Description</label>
      <textarea
        id="nt-body"
        class="in ta"
        rows="4"
        bind:value={body}
        placeholder="Markdown supported…"
      ></textarea>

      <span class="fl">Attachments</span>
      <label class="filebtn">
        <Paperclip size={14} /> Add files
        <input type="file" multiple onchange={onFiles} hidden />
      </label>
      {#if files.length}
        <div class="files">
          {#each files as f, i (f.name + i)}
            <span class="file">
              <span class="fname">{f.name}</span>
              <span class="fsize">{formatBytes(f.size)}</span>
              <button class="frm" aria-label="Remove file" onclick={() => removeFile(i)}>×</button>
            </span>
          {/each}
        </div>
      {/if}

      {#if error}<div class="err">{error}</div>{/if}

      <div class="actions">
        <Button variant="ghost" onclick={() => (open = false)}>Cancel</Button>
        <Button variant="primary" disabled={busy || !title.trim()} onclick={submit}>
          {busy ? 'Creating…' : 'Create card'}
        </Button>
      </div>
    </div>
  </div>
{/if}

<style>
  .scrim {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.4);
    z-index: 90;
  }
  .modal-wrap {
    position: fixed;
    inset: 0;
    z-index: 91;
    display: flex;
    align-items: flex-start;
    justify-content: center;
    padding: 10vh 16px 5vh;
    pointer-events: none;
    overflow-y: auto;
  }
  .modal {
    pointer-events: auto;
    width: min(480px, 94vw);
    max-height: 85vh;
    overflow-y: auto;
    background: var(--wp-card);
    border: 1px solid var(--wp-border);
    border-radius: var(--wp-r-lg);
    box-shadow: var(--wp-shadow-lift);
    padding: 18px;
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .m-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: 4px;
  }
  .m-head h3 {
    font-family: var(--wp-font-display);
    font-size: 16px;
    font-weight: 600;
  }
  .ln {
    color: var(--wp-accent);
  }
  .close {
    display: inline-flex;
    padding: 6px;
    border: none;
    background: none;
    color: var(--wp-text-muted);
    cursor: pointer;
    border-radius: var(--wp-r-sm);
  }
  .close:hover {
    background: var(--wp-elevated);
    color: var(--wp-text);
  }
  .fl {
    font-size: 12px;
    font-weight: 500;
    color: var(--wp-text-muted);
    margin-top: 6px;
  }
  .in {
    height: 34px;
    padding: 0 10px;
    border-radius: var(--wp-r-sm);
    border: 1px solid var(--wp-border);
    background: var(--wp-canvas);
    color: var(--wp-text);
    width: 100%;
  }
  .ta {
    height: auto;
    padding: 8px 10px;
    resize: vertical;
    font-family: var(--wp-font-sans);
  }
  .filebtn {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    align-self: flex-start;
    height: 30px;
    padding: 0 12px;
    border-radius: var(--wp-r-pill);
    border: 1px dashed var(--wp-border-strong);
    color: var(--wp-text-muted);
    font-size: 12px;
    cursor: pointer;
  }
  .filebtn:hover {
    color: var(--wp-text);
    background: var(--wp-elevated);
  }
  .files {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .file {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 4px 6px;
    border-radius: var(--wp-r-sm);
    background: var(--wp-surface);
    border: 1px solid var(--wp-border);
    font-size: 12px;
  }
  .fname {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .fsize {
    font-family: var(--wp-font-mono);
    font-size: 11px;
    color: var(--wp-text-subtle);
  }
  .frm {
    border: none;
    background: none;
    color: var(--wp-text-subtle);
    cursor: pointer;
    font-size: 15px;
    line-height: 1;
    padding: 0;
  }
  .frm:hover {
    color: var(--wp-text);
  }
  .err {
    font-size: 12px;
    color: var(--wp-error);
  }
  .actions {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    margin-top: 10px;
  }
</style>
