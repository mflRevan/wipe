<script lang="ts">
  import { fly, fade } from 'svelte/transition';
  import { browser } from '$app/environment';
  import { get } from 'svelte/store';
  import { X, Eye, Pencil, Plus, Send } from 'lucide-svelte';
  import Chip from './ui/Chip.svelte';
  import Avatar from './Avatar.svelte';
  import Markdown from './Markdown.svelte';
  import LabelPicker from './LabelPicker.svelte';
  import AssigneePicker from './AssigneePicker.svelte';
  import Attachments from './Attachments.svelte';
  import { api } from '$lib/api';
  import {
    board,
    definitions,
    identities,
    currentProject,
    rewinding,
    moveTicket
  } from '$lib/stores/board';
  import { formatDate } from '$lib/utils';
  import type { Ticket, TicketPatch } from '$lib/types';

  let { ticketId = $bindable(null) }: { ticketId: string | null } = $props();

  const reduced = browser && matchMedia('(prefers-reduced-motion: reduce)').matches;
  const dur = reduced ? 0 : 220;

  let ticket = $derived<Ticket | undefined>(
    $board?.lists.flatMap((l) => l.tickets).find((t) => t.id === ticketId)
  );
  let currentList = $derived(
    $board?.lists.find((l) => l.tickets.some((t) => t.id === ticketId))
  );
  let readOnly = $derived($rewinding);

  let saveError = $state<string | null>(null);

  // --- title ---
  let titleDraft = $state('');
  let titleFocused = $state(false);
  $effect(() => {
    if (!titleFocused && ticket) titleDraft = ticket.title;
  });

  // --- body ---
  let editingBody = $state(false);
  let bodyDraft = $state('');

  // --- tags ---
  let tagInput = $state('');

  // --- comments ---
  let commentDraft = $state('');

  function proj() {
    return get(currentProject) ?? undefined;
  }

  async function patch(p: TicketPatch) {
    if (!ticket) return;
    saveError = null;
    try {
      await api.patchTicket(ticket.id, p, proj());
    } catch (e) {
      saveError = e instanceof Error ? e.message : String(e);
    }
  }

  async function saveTitle() {
    titleFocused = false;
    const v = titleDraft.trim();
    if (ticket && v && v !== ticket.title) await patch({ title: v });
  }

  function startBody() {
    bodyDraft = ticket?.body ?? '';
    editingBody = true;
  }
  async function saveBody() {
    editingBody = false;
    if (ticket && bodyDraft !== (ticket.body ?? '')) await patch({ body: bodyDraft });
  }

  async function changeStatus(to: string) {
    if (!ticket || !to || to === currentList?.list) return;
    const dest = $board?.lists.find((l) => l.list === to);
    await moveTicket(ticket.id, to, dest ? dest.tickets.length : 0);
  }

  async function addTag() {
    const t = tagInput.trim();
    if (!ticket || !t || ticket.tags.includes(t)) {
      tagInput = '';
      return;
    }
    tagInput = '';
    await patch({ tags: [...ticket.tags, t] });
  }
  async function removeTag(tag: string) {
    if (!ticket) return;
    await patch({ tags: ticket.tags.filter((x) => x !== tag) });
  }

  async function addComment() {
    const b = commentDraft.trim();
    if (!ticket || !b) return;
    commentDraft = '';
    saveError = null;
    try {
      await api.addComment(ticket.id, b, undefined, proj());
    } catch (e) {
      saveError = e instanceof Error ? e.message : String(e);
    }
  }

  function identityFor(id: string) {
    return $identities.find((i) => i.id === id);
  }
  function close() {
    ticketId = null;
  }
</script>

{#if ticketId && ticket}
  <div class="scrim" transition:fade={{ duration: dur }} onclick={close} role="presentation"></div>
  <aside
    class="drawer"
    transition:fly={{ x: 420, duration: dur, opacity: 1 }}
    aria-label="Ticket details"
  >
    <div class="inner wp-scroll">
      <header class="d-head">
        <div class="idrow">
          <span class="tid">{ticket.id}</span>
          {#if readOnly}<span class="ro">read-only</span>{/if}
        </div>
        <button class="close" aria-label="Close" onclick={close}><X size={18} /></button>
      </header>

      <!-- title -->
      {#if readOnly}
        <h2 class="title-ro">{ticket.title}</h2>
      {:else}
        <input
          class="title-input"
          bind:value={titleDraft}
          onfocus={() => (titleFocused = true)}
          onblur={saveTitle}
          onkeydown={(e) => e.key === 'Enter' && e.currentTarget.blur()}
        />
      {/if}

      <!-- meta grid -->
      <div class="grid">
        <div class="field">
          <span class="flabel">Status</span>
          <select
            class="sel"
            disabled={readOnly}
            value={currentList?.list ?? ''}
            onchange={(e) => changeStatus(e.currentTarget.value)}
          >
            {#each $board?.lists ?? [] as l (l.list)}
              <option value={l.list}>{l.name}</option>
            {/each}
          </select>
        </div>

        <div class="field">
          <span class="flabel">Type</span>
          <select
            class="sel"
            disabled={readOnly}
            value={ticket.type ?? ''}
            onchange={(e) =>
              patch({ type: e.currentTarget.value === '' ? null : e.currentTarget.value })}
          >
            <option value="">— none —</option>
            {#each $definitions.types as t (t)}
              <option value={t}>{t}</option>
            {/each}
          </select>
        </div>

        <div class="field">
          <span class="flabel">Priority</span>
          <select
            class="sel"
            disabled={readOnly}
            value={ticket.priority ?? ''}
            onchange={(e) =>
              patch({ priority: e.currentTarget.value === '' ? null : e.currentTarget.value })}
          >
            <option value="">— none —</option>
            {#each $definitions.priorities as p (p)}
              <option value={p}>{p}</option>
            {/each}
          </select>
        </div>
      </div>

      <!-- labels -->
      <div class="field">
        <span class="flabel">Labels</span>
        {#if readOnly}
          <div class="chips-ro">
            {#each ticket.labels as l (l)}<Chip>{l}</Chip>{:else}<span class="dim">—</span>{/each}
          </div>
        {:else}
          <LabelPicker selected={ticket.labels} onchange={(labels) => patch({ labels })} />
        {/if}
      </div>

      <!-- tags -->
      <div class="field">
        <span class="flabel">Tags</span>
        <div class="tags">
          {#each ticket.tags as tag (tag)}
            <Chip mono onremove={readOnly ? undefined : () => removeTag(tag)}>#{tag}</Chip>
          {/each}
          {#if !readOnly}
            <input
              class="taginput"
              placeholder="add tag"
              bind:value={tagInput}
              onkeydown={(e) => e.key === 'Enter' && addTag()}
              onblur={addTag}
            />
          {:else if ticket.tags.length === 0}
            <span class="dim">—</span>
          {/if}
        </div>
      </div>

      <!-- assignees -->
      <div class="field">
        <span class="flabel">Assignees</span>
        {#if readOnly}
          <div class="chips-ro">
            {#each ticket.assignees as a (a)}
              <span class="pill-ro"
                ><Avatar id={a} identity={identityFor(a)} size={20} />{identityFor(a)?.display_name ??
                  a}</span
              >
            {:else}
              <span class="dim">—</span>
            {/each}
          </div>
        {:else}
          <AssigneePicker
            selected={ticket.assignees}
            onchange={(assignees) => patch({ assignees })}
          />
        {/if}
      </div>

      <!-- body -->
      <div class="field">
        <div class="flabel-row">
          <span class="flabel">Description</span>
          {#if !readOnly}
            {#if editingBody}
              <button class="linkbtn" onclick={saveBody}><Eye size={12} /> Preview</button>
            {:else}
              <button class="linkbtn" onclick={startBody}><Pencil size={12} /> Edit</button>
            {/if}
          {/if}
        </div>
        {#if editingBody && !readOnly}
          <textarea
            class="body-edit wp-scroll"
            bind:value={bodyDraft}
            placeholder="Markdown supported…"
          ></textarea>
        {:else if ticket.body}
          <Markdown source={ticket.body} />
        {:else}
          <span class="dim">No description.</span>
        {/if}
      </div>

      <!-- attachments -->
      <div class="field">
        <span class="flabel">Attachments</span>
        <Attachments ticketId={ticket.id} attachments={ticket.attachments} {readOnly} />
      </div>

      <!-- comments -->
      <div class="field">
        <span class="flabel">Comments ({ticket.comments.length})</span>
        <div class="comments">
          {#each ticket.comments as c (c.id)}
            <div class="comment">
              <div class="c-head">
                <Avatar id={c.author} identity={identityFor(c.author)} size={20} />
                <span class="c-author">{identityFor(c.author)?.display_name ?? c.author}</span>
                <span class="c-time">{formatDate(c.created)}{c.edited ? ' · edited' : ''}</span>
              </div>
              <div class="c-body"><Markdown source={c.body} /></div>
            </div>
          {/each}
          {#if ticket.comments.length === 0}
            <span class="dim">No comments yet.</span>
          {/if}
        </div>
        {#if !readOnly}
          <div class="c-add">
            <textarea
              class="c-input"
              rows="2"
              placeholder="Write a comment…"
              bind:value={commentDraft}
              onkeydown={(e) => {
                if (e.key === 'Enter' && (e.metaKey || e.ctrlKey)) addComment();
              }}
            ></textarea>
            <button class="c-send" aria-label="Send comment" onclick={addComment}>
              <Send size={15} />
            </button>
          </div>
        {/if}
      </div>

      {#if saveError}<div class="err">{saveError}</div>{/if}

      <footer class="d-foot">
        <span class="ts">Created {formatDate(ticket.created)} · Updated {formatDate(ticket.updated)}</span>
      </footer>
    </div>
  </aside>
{/if}

<style>
  .scrim {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.4);
    z-index: 80;
  }
  .drawer {
    position: fixed;
    top: 0;
    right: 0;
    bottom: 0;
    width: min(480px, 100vw);
    z-index: 81;
    padding: 10px;
  }
  .inner {
    height: 100%;
    overflow-y: auto;
    background: var(--wp-card);
    border: 1px solid var(--wp-border);
    border-radius: var(--wp-r-lg);
    box-shadow: var(--wp-shadow-lift);
    padding: 16px 18px;
    display: flex;
    flex-direction: column;
    gap: 16px;
  }
  .d-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }
  .idrow {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .tid {
    font-family: var(--wp-font-mono);
    font-size: 13px;
    color: var(--wp-text-subtle);
  }
  .ro {
    font-size: 10px;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    padding: 2px 6px;
    border-radius: var(--wp-r-sm);
    border: 1px solid var(--wp-border-strong);
    color: var(--wp-text-muted);
  }
  .close {
    display: inline-flex;
    padding: 6px;
    border-radius: var(--wp-r-sm);
    border: none;
    background: none;
    color: var(--wp-text-muted);
    cursor: pointer;
  }
  .close:hover {
    background: var(--wp-elevated);
    color: var(--wp-text);
  }
  .title-input {
    font-family: var(--wp-font-display);
    font-size: 18px;
    font-weight: 600;
    letter-spacing: -0.01em;
    width: 100%;
    padding: 6px 8px;
    margin: -6px -8px;
    border: 1px solid transparent;
    border-radius: var(--wp-r-sm);
    background: transparent;
    color: var(--wp-text);
  }
  .title-input:hover {
    border-color: var(--wp-border);
  }
  .title-input:focus {
    background: var(--wp-canvas);
    border-color: var(--wp-border-strong);
  }
  .title-ro {
    font-size: 18px;
    font-weight: 600;
  }
  .grid {
    display: grid;
    grid-template-columns: 1fr 1fr 1fr;
    gap: 10px;
  }
  .field {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .flabel {
    font-family: var(--wp-font-display);
    font-size: 11px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: var(--wp-text-muted);
  }
  .flabel-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }
  .sel {
    height: 32px;
    padding: 0 8px;
    border-radius: var(--wp-r-sm);
    border: 1px solid var(--wp-border);
    background: var(--wp-card);
    color: var(--wp-text);
    cursor: pointer;
  }
  .sel:disabled {
    opacity: 0.6;
    cursor: default;
  }
  .tags {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    align-items: center;
  }
  .taginput {
    height: 22px;
    width: 90px;
    padding: 0 8px;
    border-radius: var(--wp-r-sm);
    border: 1px dashed var(--wp-border-strong);
    background: transparent;
    font-size: 11px;
    color: var(--wp-text);
  }
  .chips-ro {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }
  .pill-ro {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    font-size: 12px;
    padding: 2px 8px 2px 3px;
    border-radius: var(--wp-r-pill);
    background: var(--wp-surface);
    border: 1px solid var(--wp-border);
  }
  .linkbtn {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    background: none;
    border: none;
    color: var(--wp-text-muted);
    font-size: 12px;
    cursor: pointer;
  }
  .linkbtn:hover {
    color: var(--wp-accent);
  }
  .body-edit {
    min-height: 140px;
    resize: vertical;
    padding: 10px;
    border-radius: var(--wp-r-md);
    border: 1px solid var(--wp-border);
    background: var(--wp-canvas);
    color: var(--wp-text);
    font-family: var(--wp-font-mono);
    font-size: 13px;
    line-height: 1.5;
  }
  .dim {
    color: var(--wp-text-subtle);
    font-size: 13px;
  }
  .comments {
    display: flex;
    flex-direction: column;
    gap: 10px;
  }
  .comment {
    border: 1px solid var(--wp-border);
    border-radius: var(--wp-r-md);
    padding: 8px 10px;
    background: var(--wp-surface);
  }
  .c-head {
    display: flex;
    align-items: center;
    gap: 6px;
    margin-bottom: 4px;
  }
  .c-author {
    font-size: 12px;
    font-weight: 500;
  }
  .c-time {
    margin-left: auto;
    font-family: var(--wp-font-mono);
    font-size: 11px;
    color: var(--wp-text-subtle);
  }
  .c-add {
    display: flex;
    gap: 8px;
    align-items: flex-end;
  }
  .c-input {
    flex: 1;
    resize: vertical;
    padding: 8px 10px;
    border-radius: var(--wp-r-md);
    border: 1px solid var(--wp-border);
    background: var(--wp-canvas);
    color: var(--wp-text);
    font-size: 13px;
  }
  .c-send {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    height: 36px;
    width: 36px;
    flex: none;
    border-radius: var(--wp-r-sm);
    border: none;
    background: var(--wp-accent);
    color: var(--wp-on-accent);
    cursor: pointer;
  }
  .c-send:hover {
    background: var(--wp-accent-hover);
  }
  .err {
    font-size: 12px;
    color: var(--wp-error);
  }
  .d-foot {
    border-top: 1px solid var(--wp-border);
    padding-top: 12px;
  }
  .ts {
    font-family: var(--wp-font-mono);
    font-size: 11px;
    color: var(--wp-text-subtle);
  }
</style>
