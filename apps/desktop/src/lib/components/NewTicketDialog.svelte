<script lang="ts">
  import { fade, scale } from 'svelte/transition';
  import { browser } from '$app/environment';
  import { get } from 'svelte/store';
  import { X, Paperclip, CheckSquare, ShieldCheck } from 'lucide-svelte';
  import LabelPicker from './LabelPicker.svelte';
  import AssigneePicker from './AssigneePicker.svelte';
  import LocalChecks from './LocalChecks.svelte';
  import { api } from '$lib/api';
  import { definitions, currentProject, loadBoard } from '$lib/stores/board';
  import { formatBytes, filesFromClipboard, looksLikePath } from '$lib/utils';
  import type { ChecklistItem } from '$lib/types';

  let {
    open = $bindable(false),
    listId,
    listName
  }: { open?: boolean; listId: string; listName: string } = $props();

  const reduced = browser && matchMedia('(prefers-reduced-motion: reduce)').matches;
  const dur = reduced ? 0 : 180;

  // Everything is staged locally; nothing is persisted until Create is pressed.
  let title = $state('');
  let priority = $state('');
  let body = $state('');
  let labels = $state<string[]>([]);
  let assignees = $state<string[]>([]);
  let checklist = $state<ChecklistItem[]>([]);
  let acceptance = $state<ChecklistItem[]>([]);
  let files = $state<File[]>([]);
  // Local filesystem paths pasted into the form; attached (server-read) on create.
  let pendingPaths = $state<string[]>([]);
  let error = $state<string | null>(null);
  let busy = $state(false);

  // Reset the whole form whenever the dialog (re)opens, so a previous draft never
  // leaks into a new card.
  let wasOpen = $state(false);
  $effect(() => {
    if (open && !wasOpen) reset();
    wasOpen = open;
  });

  function reset() {
    title = '';
    priority = '';
    body = '';
    labels = [];
    assignees = [];
    checklist = [];
    acceptance = [];
    files = [];
    pendingPaths = [];
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
  function removePath(i: number) {
    pendingPaths = pendingPaths.filter((_, idx) => idx !== i);
  }
  function baseName(p: string): string {
    return p.split(/[\\/]/).filter(Boolean).pop() ?? p;
  }

  // Paste media/path into the form: file/image blobs are staged like picked files;
  // a pasted local path is staged and attached (server-read) on create.
  function onPasteStage(e: ClipboardEvent) {
    const fs = filesFromClipboard(e.clipboardData);
    if (fs.length) {
      e.preventDefault();
      files = [...files, ...fs];
      return;
    }
    const text = e.clipboardData?.getData('text') ?? '';
    if (looksLikePath(text)) {
      e.preventDefault();
      const p = text.trim();
      if (!pendingPaths.includes(p)) pendingPaths = [...pendingPaths, p];
    }
  }

  function autofocus(node: HTMLTextAreaElement) {
    requestAnimationFrame(() => node.focus());
    return {};
  }

  async function submit() {
    const t = title.trim();
    if (!t || busy) return;
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
      // Persist staged checklist / acceptance items (in order), carrying their
      // done state, now that the ticket exists.
      for (const it of checklist) {
        const tk = await api.addCheckItem('checklist', ticket.id, it.text, project);
        if (it.done) {
          const last = tk.checklist[tk.checklist.length - 1];
          if (last) await api.setCheckItem('checklist', ticket.id, last.id, { done: true }, project);
        }
      }
      for (const it of acceptance) {
        const tk = await api.addCheckItem('acceptance', ticket.id, it.text, project);
        if (it.done) {
          const last = tk.acceptance[tk.acceptance.length - 1];
          if (last)
            await api.setCheckItem('acceptance', ticket.id, last.id, { done: true }, project);
        }
      }
      for (const f of files) await api.uploadAttachment(ticket.id, f, project);
      for (const p of pendingPaths) await api.attachPath(ticket.id, p, project);
      await loadBoard();
      reset();
      open = false;
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    } finally {
      busy = false;
    }
  }

  function onKey(e: KeyboardEvent) {
    if (e.key === 'Escape' && open && !e.defaultPrevented) open = false;
  }
</script>

<svelte:window onkeydown={onKey} />

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
      transition:scale={{ duration: dur, start: 0.97 }}
      role="dialog"
      aria-modal="true"
      aria-label="New card"
    >
      <button class="close" aria-label="Close" onclick={() => (open = false)}><X size={18} /></button>

      <div class="pad">
        <div class="idrow">
          <span class="listtag">{listName}</span>
          <span class="tid">New card</span>
        </div>

        <!-- svelte-ignore a11y_autofocus -->
        <textarea
          class="title-input"
          rows="1"
          placeholder="Card title…"
          bind:value={title}
          use:autofocus
          onpaste={onPasteStage}
          onkeydown={(e) => {
            if (e.key === 'Enter') {
              e.preventDefault();
              void submit();
            }
          }}
        ></textarea>

        <div class="two-col">
          <div class="field">
            <span class="flabel">Labels</span>
            <LabelPicker selected={labels} onchange={(v) => (labels = v)} />
          </div>
          <div class="field">
            <span class="flabel">Members</span>
            <AssigneePicker selected={assignees} onchange={(v) => (assignees = v)} />
          </div>
        </div>

        <div class="field">
          <span class="flabel">Priority</span>
          <select class="in" bind:value={priority}>
            <option value="">— none —</option>
            {#each $definitions.priorities as p (p)}<option value={p}>{p}</option>{/each}
          </select>
        </div>

        <div class="field">
          <span class="flabel">Description</span>
          <textarea
            class="in ta"
            rows="4"
            bind:value={body}
            onpaste={onPasteStage}
            placeholder="Markdown supported…"
          ></textarea>
        </div>

        <LocalChecks
          bind:items={checklist}
          label="Checklist"
          icon={CheckSquare}
          placeholder="Add an item…"
        />
        <LocalChecks
          bind:items={acceptance}
          label="Acceptance criteria"
          icon={ShieldCheck}
          placeholder="Add a criterion…"
          accent
        />

        <div class="field">
          <span class="flabel">Attachments</span>
          <label class="filebtn">
            <Paperclip size={14} /> Add files
            <input type="file" multiple onchange={onFiles} hidden />
          </label>
          {#if files.length || pendingPaths.length}
            <div class="files">
              {#each files as f, i (f.name + i)}
                <span class="file">
                  <span class="fname">{f.name}</span>
                  <span class="fsize">{formatBytes(f.size)}</span>
                  <button class="frm" aria-label="Remove file" onclick={() => removeFile(i)}
                    >×</button
                  >
                </span>
              {/each}
              {#each pendingPaths as p, i (p + i)}
                <span class="file">
                  <span class="fname">{baseName(p)}</span>
                  <span class="fsize">path</span>
                  <button class="frm" aria-label="Remove path" onclick={() => removePath(i)}
                    >×</button
                  >
                </span>
              {/each}
            </div>
          {/if}
        </div>

        {#if error}<div class="err">{error}</div>{/if}
      </div>

      <!-- Floating Create/Cancel bar pinned to the popup's bottom-right border. -->
      <div class="foot">
        <button class="fb ghost" onclick={() => (open = false)}>Cancel</button>
        <button class="fb primary" disabled={busy || !title.trim()} onclick={submit}>
          {busy ? 'Creating…' : 'Create card'}
        </button>
      </div>
    </div>
  </div>
{/if}

<style>
  .scrim {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.5);
    z-index: 80;
  }
  .modal-wrap {
    position: fixed;
    inset: 0;
    z-index: 81;
    display: flex;
    align-items: flex-start;
    justify-content: center;
    padding: 6vh 16px 4vh;
    pointer-events: none;
    overflow-y: auto;
  }
  .modal {
    position: relative;
    pointer-events: auto;
    width: min(640px, 100%);
    max-height: 88vh;
    overflow-y: auto;
    background: var(--wp-card);
    border: 1px solid var(--wp-border);
    border-radius: var(--wp-r-lg);
    box-shadow: var(--wp-shadow-lift);
  }
  .close {
    position: absolute;
    top: 12px;
    right: 12px;
    z-index: 2;
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
  .pad {
    display: flex;
    flex-direction: column;
    gap: 16px;
    padding: 20px 22px 18px;
  }
  .idrow {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .listtag {
    padding: 2px 8px;
    border-radius: var(--wp-r-pill);
    background: color-mix(in srgb, var(--wp-accent) 14%, transparent);
    color: var(--wp-accent);
    font-size: 12px;
    font-weight: 600;
  }
  .tid {
    font-family: var(--wp-font-mono);
    font-size: 12px;
    color: var(--wp-text-subtle);
  }
  .title-input {
    width: 100%;
    border: none;
    background: none;
    resize: none;
    overflow: hidden;
    color: var(--wp-text);
    font-family: var(--wp-font-display);
    font-size: 22px;
    font-weight: 600;
    line-height: 1.3;
    padding: 0;
  }
  .title-input:focus {
    outline: none;
  }
  .two-col {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 16px;
  }
  @media (max-width: 520px) {
    .two-col {
      grid-template-columns: 1fr;
    }
  }
  .field {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .flabel {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    width: fit-content;
    font-family: var(--wp-font-display);
    font-size: 11px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: var(--wp-text-muted);
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
    line-height: 1.5;
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
  /* Sticky action bar, aligned to the popup's bottom-right border. */
  .foot {
    position: sticky;
    bottom: 0;
    display: flex;
    justify-content: flex-end;
    gap: 10px;
    padding: 12px 22px;
    background: linear-gradient(
      to bottom,
      color-mix(in srgb, var(--wp-card) 0%, transparent),
      var(--wp-card) 45%
    );
    border-top: 1px solid var(--wp-border);
    border-radius: 0 0 var(--wp-r-lg) var(--wp-r-lg);
  }
  .fb {
    height: 34px;
    padding: 0 16px;
    border-radius: var(--wp-r-sm);
    font-size: 13px;
    font-weight: 600;
    cursor: pointer;
    border: 1px solid transparent;
    transition:
      background var(--wp-fast) var(--wp-ease),
      border-color var(--wp-fast) var(--wp-ease),
      opacity var(--wp-fast) var(--wp-ease);
  }
  .fb.ghost {
    background: none;
    border-color: var(--wp-border-strong);
    color: var(--wp-text-muted);
  }
  .fb.ghost:hover {
    background: var(--wp-elevated);
    color: var(--wp-text);
  }
  .fb.primary {
    background: var(--wp-accent);
    color: var(--wp-on-accent, #fff);
  }
  .fb.primary:hover:not(:disabled) {
    filter: brightness(1.05);
  }
  .fb.primary:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }
</style>
