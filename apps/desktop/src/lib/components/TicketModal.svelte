<script lang="ts">
  import { fade, scale } from 'svelte/transition';
  import { browser } from '$app/environment';
  import { get } from 'svelte/store';
  import { X, Pencil, Send, Flag, Check, CheckSquare, ShieldCheck } from 'lucide-svelte';
  import Chip from './ui/Chip.svelte';
  import Avatar from './Avatar.svelte';
  import Markdown from './Markdown.svelte';
  import LabelPicker from './LabelPicker.svelte';
  import AssigneePicker from './AssigneePicker.svelte';
  import Attachments from './Attachments.svelte';
  import ChecksSection from './ChecksSection.svelte';
  import { api, mediaUrl } from '$lib/api';
  import {
    board,
    definitions,
    identities,
    currentProject,
    rewinding,
    markSelfChange,
    applyTicket
  } from '$lib/stores/board';
  import { formatDate, mediaKind, priorityColor, activityPhrase } from '$lib/utils';
  import type { Activity, Attachment, Comment, Ticket, TicketPatch } from '$lib/types';

  type FeedItem =
    | { type: 'comment'; ts: string; comment: Comment }
    | { type: 'event'; ts: string; event: Activity };

  let { ticketId = $bindable(null) }: { ticketId: string | null } = $props();

  const reduced = browser && matchMedia('(prefers-reduced-motion: reduce)').matches;
  const dur = reduced ? 0 : 200;

  let ticket = $derived<Ticket | undefined>(
    $board?.lists.flatMap((l) => l.tickets).find((t) => t.id === ticketId)
  );
  let currentList = $derived($board?.lists.find((l) => l.tickets.some((t) => t.id === ticketId)));
  let readOnly = $derived($rewinding);

  // First image attachment becomes the modal cover.
  let cover = $derived<Attachment | undefined>(
    ticket?.attachments.find((a) => mediaKind(a.mime, a.name) === 'image')
  );

  // Comments and activity events, interleaved and sorted newest-first - the
  // Trello-style timeline in the right pane.
  let feed = $derived<FeedItem[]>(
    ticket
      ? [
          ...ticket.comments.map(
            (c): FeedItem => ({ type: 'comment', ts: c.created, comment: c })
          ),
          ...ticket.activity.map((a): FeedItem => ({ type: 'event', ts: a.ts, event: a }))
        ].sort((x, y) => new Date(y.ts).getTime() - new Date(x.ts).getTime())
      : []
  );

  let saveError = $state<string | null>(null);

  function proj() {
    return get(currentProject) ?? undefined;
  }

  async function patch(p: TicketPatch) {
    if (!ticket) return;
    saveError = null;
    // Your own edits from this modal shouldn't flash on the board as "activity" -
    // suppress the highlight and reflect the server's result immediately.
    markSelfChange(ticket.id);
    try {
      applyTicket(await api.patchTicket(ticket.id, p, proj()));
    } catch (e) {
      saveError = e instanceof Error ? e.message : String(e);
    }
  }

  // --- title ---
  let titleDraft = $state('');
  let titleFocused = $state(false);
  $effect(() => {
    if (!titleFocused && ticket) titleDraft = ticket.title;
  });

  /** Auto-grow the title textarea so long titles wrap instead of truncating. */
  function autosize(node: HTMLTextAreaElement, _dep?: unknown) {
    const fit = () => {
      node.style.height = 'auto';
      node.style.height = `${node.scrollHeight}px`;
    };
    fit();
    node.addEventListener('input', fit);
    return { update: fit, destroy: () => node.removeEventListener('input', fit) };
  }

  async function saveTitle() {
    titleFocused = false;
    // The title is single-line semantically; collapse any pasted newlines.
    const v = titleDraft.replace(/\s*\n\s*/g, ' ').trim();
    if (ticket && v && v !== ticket.title) await patch({ title: v });
  }

  // --- body (description) ---
  //
  // Editing is per-ticket and deliberate: the editor belongs to exactly one
  // ticket (`editingFor`), a save only ever targets that ticket, and leaving
  // without saving (click outside, Escape, refresh) stashes the text as a local
  // draft - shown with a "draft" pill and prefilled on the next Edit, but never
  // written to the ticket and never producing activity.
  let editingBody = $state(false);
  let bodyDraft = $state('');
  let hasDraft = $state(false);
  let editingFor: string | null = null;

  function draftKey(id: string) {
    return `wipe:draft:${get(currentProject) ?? ''}:${id}`;
  }
  function readDraft(id: string): string | null {
    try {
      return localStorage.getItem(draftKey(id));
    } catch {
      return null;
    }
  }
  function writeDraft(id: string, text: string | null) {
    try {
      if (text === null) localStorage.removeItem(draftKey(id));
      else localStorage.setItem(draftKey(id), text);
    } catch {
      /* drafts are best-effort */
    }
  }

  // Per-ticket reset: the popup ALWAYS opens in preview mode and no edit state
  // survives switching tickets (previously the open editor could save one
  // ticket's text into another).
  let openedFor = $state<string | null>(null);
  $effect(() => {
    if (ticketId === openedFor) return;
    openedFor = ticketId;
    editingBody = false;
    editingFor = null;
    bodyDraft = '';
    saveError = null;
    commentDraft = '';
    titleFocused = false;
    hasDraft = ticketId ? readDraft(ticketId) !== null : false;
  });

  // While the editor is open, continuously stash the draft so a refresh or an
  // accidental close never loses text. A draft identical to the saved body is
  // not a draft - drop it so no stale "draft" pill lingers.
  $effect(() => {
    if (!editingBody || !editingFor) return;
    writeDraft(editingFor, bodyDraft === (ticket?.body ?? '') ? null : bodyDraft);
  });

  function startBody() {
    if (!ticket) return;
    bodyDraft = readDraft(ticket.id) ?? ticket.body ?? '';
    editingFor = ticket.id;
    editingBody = true;
  }
  /** Deliberate save: writes the ticket (records activity) and clears the draft. */
  async function saveBody() {
    const target = editingFor;
    editingBody = false;
    editingFor = null;
    // Hardening: never let a leftover editor write into a different ticket.
    if (!ticket || !target || target !== ticket.id) return;
    writeDraft(target, null);
    hasDraft = false;
    if (bodyDraft !== (ticket.body ?? '')) await patch({ body: bodyDraft });
  }
  /** Explicit discard: drops the edits and the cached draft. */
  function cancelBody() {
    if (editingFor) writeDraft(editingFor, null);
    hasDraft = false;
    editingBody = false;
    editingFor = null;
  }
  /** Leave edit mode keeping the draft cached (click outside / Escape / close). */
  function stashBody() {
    editingBody = false;
    if (editingFor) hasDraft = readDraft(editingFor) !== null;
    editingFor = null;
  }

  // --- comments ---
  let commentDraft = $state('');

  async function addComment() {
    const b = commentDraft.trim();
    if (!ticket || !b) return;
    commentDraft = '';
    saveError = null;
    markSelfChange(ticket.id);
    try {
      await api.addComment(ticket.id, b, undefined, proj());
    } catch (e) {
      saveError = e instanceof Error ? e.message : String(e);
    }
  }

  async function deleteComment(commentId: string) {
    if (!ticket) return;
    saveError = null;
    markSelfChange(ticket.id);
    try {
      applyTicket(await api.deleteComment(ticket.id, commentId, proj()));
    } catch (e) {
      saveError = e instanceof Error ? e.message : String(e);
    }
  }

  function identityFor(id: string) {
    return $identities.find((i) => i.id === id);
  }
  // Activity actors and CLI comment authors are stored as the full git identity
  // (`Name <email>`), while identities are keyed by email - so resolve either form.
  function identityForActor(actor: string) {
    const direct = identityFor(actor);
    if (direct) return direct;
    const m = actor.match(/<([^>]+)>/);
    return m ? identityFor(m[1]) : undefined;
  }
  function displayActor(actor: string) {
    const found = identityForActor(actor);
    if (found) return found.display_name;
    const m = actor.match(/^(.*?)\s*<[^>]+>$/);
    return m && m[1] ? m[1] : actor;
  }
  function resolveName(id: string) {
    return identityFor(id)?.display_name ?? displayActor(id);
  }
  function close() {
    // Closing with the editor open stashes the draft (it is NOT saved).
    if (editingBody) stashBody();
    ticketId = null;
  }
  function onKey(e: KeyboardEvent) {
    if (e.key !== 'Escape' || !ticketId) return;
    // An inner control (open popover, a checklist-item edit input) that handled the
    // Escape marks it defaultPrevented - don't also close the whole modal then.
    if (e.defaultPrevented) return;
    // First Escape leaves the editor (draft kept); the next closes the popup.
    if (editingBody) stashBody();
    else close();
  }
</script>

<svelte:window onkeydown={onKey} />

{#if ticketId && ticket}
  <div class="scrim" transition:fade={{ duration: dur }} onclick={close} role="presentation"></div>
  <div class="modal-wrap" role="dialog" aria-modal="true" aria-label="Ticket details">
    <div
      class="modal wp-scroll"
      transition:scale={{ duration: dur, start: 0.97, opacity: 0 }}
    >
      <!-- cover -->
      {#if cover}
        <div class="cover">
          <img src={mediaUrl(cover.path, proj())} alt={cover.name} />
        </div>
      {/if}

      <button class="close" aria-label="Close" onclick={close}><X size={18} /></button>

      <div class="pad">
       <div class="main">
        <div class="idrow">
          {#if currentList}<span class="listtag">{currentList.name}</span>{/if}
          <span class="tid">{ticket.id}</span>
          {#if readOnly}<span class="ro">read-only</span>{/if}
        </div>

        <!-- title (auto-grows and wraps; Enter commits) -->
        {#if readOnly}
          <h2 class="title-ro">{ticket.title}</h2>
        {:else}
          <textarea
            class="title-input"
            rows="1"
            bind:value={titleDraft}
            use:autosize={titleDraft}
            onfocus={() => (titleFocused = true)}
            onblur={saveTitle}
            onkeydown={(e) => {
              if (e.key === 'Enter') {
                e.preventDefault();
                e.currentTarget.blur();
              }
            }}
          ></textarea>
        {/if}

        <!-- labels + members -->
        <div class="two-col">
          <div class="field">
            <span class="flabel">Labels</span>
            {#if readOnly}
              <div class="chips-ro">
                {#each ticket.labels as l (l)}<Chip>{l}</Chip>{:else}<span class="dim">-</span>{/each}
              </div>
            {:else}
              <LabelPicker selected={ticket.labels} onchange={(labels) => patch({ labels })} />
            {/if}
          </div>

          <div class="field">
            <span class="flabel">Members</span>
            {#if readOnly}
              <div class="chips-ro">
                {#each ticket.assignees as a (a)}
                  <span class="pill-ro">
                    <Avatar id={a} identity={identityFor(a)} size={20} />{identityFor(a)
                      ?.display_name ?? a}
                  </span>
                {:else}
                  <span class="dim">-</span>
                {/each}
              </div>
            {:else}
              <AssigneePicker
                selected={ticket.assignees}
                onchange={(assignees) => patch({ assignees })}
              />
            {/if}
          </div>
        </div>

        <!-- priority -->
        <div class="field">
          <span class="flabel">Priority</span>
          <div class="prio-row">
            {#if ticket.priority}
              <span class="prio-dot" style="--d:{priorityColor(ticket.priority)}"></span>
            {:else}
              <Flag size={13} class="flag" />
            {/if}
            {#if readOnly}
              <span class="prio-val">{ticket.priority ?? '-'}</span>
            {:else}
              <select
                class="sel"
                value={ticket.priority ?? ''}
                onchange={(e) =>
                  patch({ priority: e.currentTarget.value === '' ? null : e.currentTarget.value })}
              >
                <option value="">- none -</option>
                {#each $definitions.priorities as p (p)}
                  <option value={p}>{p}</option>
                {/each}
              </select>
            {/if}
          </div>
        </div>

        <!-- description -->
        <div class="field">
          <div class="flabel-row">
            <span class="flabel">Description</span>
            {#if !readOnly}
              <div class="edit-actions">
                {#if editingBody}
                  <button class="linkbtn" onclick={cancelBody}>Cancel</button>
                  <button class="savebtn" onclick={saveBody}><Check size={12} /> Save</button>
                {:else}
                  {#if hasDraft}
                    <span class="draft-pill" title="You have an unsaved draft - Edit resumes it"
                      >draft</span
                    >
                  {/if}
                  <button class="linkbtn" onclick={startBody}><Pencil size={12} /> Edit</button>
                {/if}
              </div>
            {/if}
          </div>
          {#if editingBody && !readOnly}
            <!-- svelte-ignore a11y_autofocus -->
            <textarea
              class="body-edit wp-scroll"
              autofocus
              bind:value={bodyDraft}
              placeholder="Markdown supported…"
            ></textarea>
          {:else if ticket.body}
            <button class="body-view" onclick={() => !readOnly && startBody()}>
              <Markdown source={ticket.body} />
            </button>
          {:else if !readOnly}
            <button class="body-empty" onclick={startBody}>
              {hasDraft ? 'Resume your description draft…' : 'Add a more detailed description…'}
            </button>
          {:else}
            <span class="dim">No description.</span>
          {/if}
        </div>

        <!-- checklist (worker to-dos) -->
        <ChecksSection
          kind="checklist"
          label="Checklist"
          icon={CheckSquare}
          ticketId={ticket.id}
          items={ticket.checklist}
          {readOnly}
          placeholder="Add an item…"
          onerror={(m) => (saveError = m)}
        />

        <!-- acceptance criteria (what a reviewer ticks to accept the work) -->
        <ChecksSection
          kind="acceptance"
          label="Acceptance criteria"
          icon={ShieldCheck}
          ticketId={ticket.id}
          items={ticket.acceptance}
          {readOnly}
          placeholder="Add a criterion…"
          onerror={(m) => (saveError = m)}
        />

        <!-- attachments -->
        <div class="field">
          <span class="flabel">Attachments</span>
          <Attachments ticketId={ticket.id} attachments={ticket.attachments} {readOnly} />
        </div>

        {#if saveError}<div class="err">{saveError}</div>{/if}

        <footer class="d-foot">
          <span class="ts"
            >Created {formatDate(ticket.created)} · Updated {formatDate(ticket.updated)}</span
          >
        </footer>
       </div>

       <!-- activity: comments + events, interleaved, newest first -->
       <aside class="side">
         <div class="side-head">Activity</div>
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
         <div class="feed">
           {#each feed as item (item.type + item.ts + (item.type === 'comment' ? item.comment.id : item.event.kind + item.event.detail))}
             {#if item.type === 'comment'}
               <div class="comment">
                 <div class="c-head">
                   <Avatar
                     id={identityForActor(item.comment.author)?.id ?? item.comment.author}
                     identity={identityForActor(item.comment.author)}
                     size={22}
                   />
                   <span class="c-author">{displayActor(item.comment.author)}</span>
                   <span class="c-time"
                     >{formatDate(item.comment.created)}{item.comment.edited
                       ? ' · edited'
                       : ''}</span
                   >
                   {#if !readOnly}
                     <button
                       class="c-del"
                       aria-label="Delete comment"
                       title="Delete comment"
                       onclick={() => deleteComment(item.comment.id)}><X size={13} /></button
                     >
                   {/if}
                 </div>
                 <div class="c-body"><Markdown source={item.comment.body} /></div>
               </div>
             {:else}
               <div class="event">
                 <Avatar
                   id={identityForActor(item.event.actor)?.id ?? item.event.actor}
                   identity={identityForActor(item.event.actor)}
                   size={22}
                 />
                 <div class="e-text">
                   <span class="e-actor">{displayActor(item.event.actor)}</span>
                   {activityPhrase(item.event.kind, item.event.detail, resolveName)}
                   <span class="e-time">{formatDate(item.event.ts)}</span>
                 </div>
               </div>
             {/if}
           {/each}
           {#if feed.length === 0}
             <span class="dim">No activity yet.</span>
           {/if}
         </div>
       </aside>
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
    padding: 5vh 16px;
    pointer-events: none;
    overflow-y: auto;
  }
  .modal {
    position: relative;
    pointer-events: auto;
    width: min(920px, 100%);
    max-height: 90vh;
    overflow-y: auto;
    background: var(--wp-card);
    border: 1px solid var(--wp-border);
    border-radius: var(--wp-r-lg);
    box-shadow: var(--wp-shadow-lift);
  }
  .cover {
    width: 100%;
    max-height: 260px;
    overflow: hidden;
    border-radius: var(--wp-r-lg) var(--wp-r-lg) 0 0;
    background: var(--wp-surface);
  }
  .cover img {
    display: block;
    width: 100%;
    max-height: 260px;
    object-fit: cover;
  }
  .close {
    position: absolute;
    top: 12px;
    right: 12px;
    z-index: 2;
    display: inline-flex;
    padding: 6px;
    border-radius: var(--wp-r-sm);
    border: none;
    background: color-mix(in srgb, var(--wp-card) 70%, transparent);
    backdrop-filter: blur(4px);
    color: var(--wp-text-muted);
    cursor: pointer;
  }
  .close:hover {
    background: var(--wp-elevated);
    color: var(--wp-text);
  }
  .pad {
    padding: 20px 24px 22px;
    display: grid;
    grid-template-columns: minmax(0, 1fr) 320px;
    gap: 24px;
  }
  .main {
    display: flex;
    flex-direction: column;
    gap: 18px;
    min-width: 0;
  }
  .side {
    display: flex;
    flex-direction: column;
    gap: 12px;
    border-left: 1px solid var(--wp-border);
    padding-left: 20px;
  }
  @media (max-width: 720px) {
    .pad {
      grid-template-columns: 1fr;
    }
    .side {
      border-left: none;
      border-top: 1px solid var(--wp-border);
      padding-left: 0;
      padding-top: 16px;
    }
  }
  .side-head {
    font-family: var(--wp-font-display);
    font-size: 11px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: var(--wp-text-muted);
  }
  .feed {
    display: flex;
    flex-direction: column;
    gap: 12px;
  }
  .event {
    display: flex;
    align-items: flex-start;
    gap: 8px;
  }
  .e-text {
    font-size: 12.5px;
    line-height: 1.45;
    color: var(--wp-text-muted);
    padding-top: 2px;
  }
  .e-actor {
    font-weight: 600;
    color: var(--wp-text);
  }
  .e-time {
    display: block;
    margin-top: 1px;
    font-family: var(--wp-font-mono);
    font-size: 10.5px;
    color: var(--wp-text-subtle);
  }
  .idrow {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .listtag {
    font-family: var(--wp-font-display);
    font-size: 10px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    padding: 2px 8px;
    border-radius: var(--wp-r-pill);
    background: color-mix(in srgb, var(--wp-accent) 14%, transparent);
    color: var(--wp-accent);
  }
  .tid {
    font-family: var(--wp-font-mono);
    font-size: 12px;
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
  .title-input {
    font-family: var(--wp-font-display);
    font-size: 22px;
    font-weight: 600;
    letter-spacing: -0.01em;
    line-height: 1.3;
    width: 100%;
    padding: 6px 8px;
    margin: -6px -8px;
    border: 1px solid transparent;
    border-radius: var(--wp-r-sm);
    background: transparent;
    color: var(--wp-text);
    /* A wrapping auto-grow field (not a single-line input): long titles break
       onto new lines instead of scrolling out of view. */
    resize: none;
    overflow: hidden;
    display: block;
  }
  .title-input:hover {
    border-color: var(--wp-border);
  }
  .title-input:focus {
    background: var(--wp-canvas);
    border-color: var(--wp-border-strong);
  }
  .title-ro {
    font-size: 22px;
    font-weight: 600;
    line-height: 1.3;
    overflow-wrap: anywhere;
  }
  .edit-actions {
    display: inline-flex;
    align-items: center;
    gap: 10px;
  }
  .savebtn {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    height: 24px;
    padding: 0 10px;
    border-radius: var(--wp-r-sm);
    border: none;
    background: var(--wp-accent);
    color: var(--wp-on-accent);
    font-size: 12px;
    font-weight: 500;
    cursor: pointer;
  }
  .savebtn:hover {
    background: var(--wp-accent-hover);
  }
  .draft-pill {
    font-family: var(--wp-font-mono);
    font-size: 10px;
    text-transform: uppercase;
    letter-spacing: 0.04em;
    padding: 1px 6px;
    border-radius: var(--wp-r-pill);
    background: color-mix(in srgb, var(--wp-accent) 14%, transparent);
    color: var(--wp-accent);
  }
  .two-col {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 16px;
  }
  @media (max-width: 560px) {
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
  .flabel-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }
  .prio-row {
    display: inline-flex;
    align-items: center;
    gap: 8px;
  }
  .prio-dot {
    width: 9px;
    height: 9px;
    border-radius: 50%;
    background: var(--d);
    flex: none;
  }
  :global(.prio-row .flag) {
    color: var(--wp-text-subtle);
  }
  .prio-val {
    text-transform: capitalize;
    font-size: 14px;
  }
  .sel {
    height: 32px;
    padding: 0 8px;
    border-radius: var(--wp-r-sm);
    border: 1px solid var(--wp-border);
    background: var(--wp-card);
    color: var(--wp-text);
    cursor: pointer;
    text-transform: capitalize;
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
    min-height: 150px;
    resize: vertical;
    padding: 12px;
    border-radius: var(--wp-r-md);
    border: 1px solid var(--wp-border);
    background: var(--wp-canvas);
    color: var(--wp-text);
    font-family: var(--wp-font-mono);
    font-size: 13px;
    line-height: 1.5;
  }
  .body-view {
    text-align: left;
    background: none;
    border: 1px solid transparent;
    border-radius: var(--wp-r-md);
    padding: 8px 10px;
    margin: -8px -10px;
    cursor: text;
    color: inherit;
  }
  .body-view:hover {
    background: var(--wp-surface);
  }
  .body-empty {
    text-align: left;
    padding: 12px;
    border-radius: var(--wp-r-md);
    border: 1px solid var(--wp-border);
    background: var(--wp-surface);
    color: var(--wp-text-subtle);
    font-size: 13px;
    cursor: text;
  }
  .body-empty:hover {
    background: var(--wp-elevated);
    color: var(--wp-text-muted);
  }
  .dim {
    color: var(--wp-text-subtle);
    font-size: 13px;
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
  .c-del {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 22px;
    height: 22px;
    border: none;
    border-radius: var(--wp-r-sm);
    background: none;
    color: var(--wp-text-subtle);
    cursor: pointer;
    opacity: 0;
    transition: all var(--wp-fast) var(--wp-ease);
  }
  .comment:hover .c-del {
    opacity: 1;
  }
  .c-del:hover {
    background: var(--wp-elevated);
    color: var(--wp-error);
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
