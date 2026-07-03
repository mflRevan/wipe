<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { get } from 'svelte/store';
  import { MessageSquarePlus, Search, Send, Trash2, CornerDownRight, X } from 'lucide-svelte';
  import Avatar from './Avatar.svelte';
  import Markdown from './Markdown.svelte';
  import Chip from './ui/Chip.svelte';
  import { api, subscribeChanges } from '$lib/api';
  import { currentProject, identities, definitions } from '$lib/stores/board';
  import { formatDate, labelColorFor } from '$lib/utils';
  import type { ForumMatch, ForumPost, ForumThread, ForumThreadSummary } from '$lib/types';

  let threads = $state<ForumThreadSummary[]>([]);
  let selectedId = $state<string | null>(null);
  let thread = $state<ForumThread | null>(null);
  let error = $state<string | null>(null);

  // search
  let query = $state('');
  let results = $state<ForumMatch[] | null>(null);

  // composers
  let composingThread = $state(false);
  let ntTitle = $state('');
  let ntBody = $state('');
  let replyTo = $state<string | null>(null);
  let replyBody = $state('');

  function proj() {
    return get(currentProject) ?? undefined;
  }
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

  async function loadThreads() {
    try {
      threads = await api.forumThreads(proj());
      error = null;
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    }
  }

  async function openThread(id: string) {
    selectedId = id;
    results = null;
    replyTo = id;
    try {
      thread = await api.forumThread(id, proj());
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    }
  }

  async function refresh() {
    await loadThreads();
    if (selectedId) {
      // Thread may have been deleted out from under us.
      if (threads.some((t) => t.id === selectedId)) await openThread(selectedId);
      else {
        selectedId = null;
        thread = null;
      }
    }
    if (query.trim()) await runSearch();
  }

  async function runSearch() {
    const q = query.trim();
    if (!q) {
      results = null;
      return;
    }
    try {
      results = await api.forumSearch({ q }, proj());
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    }
  }

  async function createThread() {
    const title = ntTitle.trim();
    if (!title) return;
    try {
      const t = await api.forumPost({ title, body: ntBody.trim() || undefined }, proj());
      ntTitle = '';
      ntBody = '';
      composingThread = false;
      await loadThreads();
      await openThread(t.id);
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    }
  }

  async function sendReply() {
    const body = replyBody.trim();
    if (!body || !replyTo) return;
    try {
      await api.forumReply(replyTo, { body }, proj());
      replyBody = '';
      if (selectedId) await openThread(selectedId);
      await loadThreads();
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    }
  }

  async function del(id: string) {
    try {
      await api.forumDelete(id, proj());
      if (id === selectedId) {
        selectedId = null;
        thread = null;
      }
      await refresh();
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    }
  }

  // Flatten a thread into depth-annotated posts for rendering.
  function flatten(root: ForumPost, depth = 0, acc: { post: ForumPost; depth: number }[] = []) {
    acc.push({ post: root, depth });
    for (const r of root.replies) flatten(r, depth + 1, acc);
    return acc;
  }
  let posts = $derived(thread ? flatten(thread.root) : []);

  // Reload (and reset selection) whenever the current project changes; this also
  // performs the initial load on mount.
  $effect(() => {
    void $currentProject;
    selectedId = null;
    thread = null;
    results = null;
    query = '';
    void loadThreads();
  });

  let unsub: (() => void) | null = null;
  onMount(() => {
    unsub = subscribeChanges(() => void refresh());
  });
  onDestroy(() => unsub?.());
</script>

<div class="forum">
  <aside class="side wp-scroll">
    <div class="side-head">
      <span class="h">Forum</span>
      <button class="new" onclick={() => (composingThread = !composingThread)}>
        <MessageSquarePlus size={15} /> New
      </button>
    </div>

    {#if composingThread}
      <div class="composer">
        <input class="in" placeholder="Thread title" bind:value={ntTitle} />
        <textarea class="in ta" rows="3" placeholder="Say something… (Markdown)" bind:value={ntBody}
        ></textarea>
        <div class="crow">
          <button class="ghost" onclick={() => (composingThread = false)}>Cancel</button>
          <button class="prim" disabled={!ntTitle.trim()} onclick={createThread}>Post thread</button>
        </div>
      </div>
    {/if}

    <div class="searchbar">
      <Search size={14} />
      <input
        class="sin"
        placeholder="Search the forum…"
        bind:value={query}
        onkeydown={(e) => e.key === 'Enter' && runSearch()}
        oninput={() => !query.trim() && (results = null)}
      />
    </div>

    {#if results !== null}
      <div class="reslabel">{results.length} match(es)</div>
      {#each results as r (r.id)}
        <button class="threadrow" onclick={() => openThread(r.thread_id)}>
          <div class="tr-top">
            <span class="tr-id">{r.id}</span>
            <span class="tr-author">{displayName(r.author)}</span>
          </div>
          <div class="tr-snip">{r.body}</div>
        </button>
      {/each}
    {:else}
      {#each threads as t (t.id)}
        <button
          class="threadrow"
          class:active={t.id === selectedId}
          onclick={() => openThread(t.id)}
        >
          <div class="tr-top">
            <span class="tr-id">{t.id}</span>
            <span class="tr-count">{t.posts}</span>
          </div>
          <div class="tr-title">{t.title}</div>
          <div class="tr-meta">{displayName(t.author)}</div>
        </button>
      {:else}
        <div class="empty">No threads yet. Start one with <b>New</b>.</div>
      {/each}
    {/if}
  </aside>

  <section class="thread">
    {#if error}<div class="err">{error}</div>{/if}
    {#if thread}
      <header class="th-head">
        <h2>{thread.title}</h2>
        <span class="th-id">{thread.id}</span>
      </header>
      <div class="feed wp-scroll">
        {#each posts as { post, depth } (post.id)}
          <div class="post" style="--indent:{Math.min(depth, 6)}">
            {#if depth > 0}<span class="thread-line"></span>{/if}
            <Avatar
              id={identityFor(post.author)?.id ?? post.author}
              identity={identityFor(post.author)}
              size={26}
            />
            <div class="bubble">
              <div class="p-head">
                <span class="p-author">{displayName(post.author)}</span>
                <span class="p-id">{post.id}</span>
                <span class="p-time">{formatDate(post.created)}{post.edited ? ' · edited' : ''}</span>
                <div class="p-actions">
                  <button
                    class="pa"
                    title="Reply"
                    onclick={() => {
                      replyTo = post.id;
                      replyBody = '';
                    }}><CornerDownRight size={13} /></button
                  >
                  <button class="pa danger" title="Delete (and replies)" onclick={() => del(post.id)}>
                    <Trash2 size={13} />
                  </button>
                </div>
              </div>
              {#if post.labels.length}
                <div class="p-labels">
                  {#each post.labels as l (l)}
                    <Chip color={labelColorFor(l, $definitions.labels)}>{l}</Chip>
                  {/each}
                </div>
              {/if}
              <div class="p-body"><Markdown source={post.body} /></div>
            </div>
          </div>
        {/each}
      </div>
      <div class="replybar">
        {#if replyTo && replyTo !== thread.id}
          <div class="replying">
            replying to {replyTo}
            <button class="rx" aria-label="Reply to thread instead" onclick={() => (replyTo = thread!.id)}>
              <X size={12} />
            </button>
          </div>
        {/if}
        <div class="c-add">
          <textarea
            class="c-input"
            rows="2"
            placeholder={replyTo === thread.id ? 'Reply to this thread…' : `Reply to ${replyTo}…`}
            bind:value={replyBody}
            onkeydown={(e) => {
              if (e.key === 'Enter' && (e.metaKey || e.ctrlKey)) sendReply();
            }}
          ></textarea>
          <button class="c-send" aria-label="Send reply" onclick={sendReply}><Send size={15} /></button>
        </div>
      </div>
    {:else}
      <div class="pick">Select a thread, or start a new one.</div>
    {/if}
  </section>
</div>

<style>
  .forum {
    display: grid;
    grid-template-columns: 300px minmax(0, 1fr);
    gap: 12px;
    height: 100%;
    min-height: 0;
  }
  .side {
    display: flex;
    flex-direction: column;
    gap: 8px;
    overflow-y: auto;
    padding-right: 4px;
    border-right: 1px solid var(--wp-border);
  }
  .side-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }
  .side-head .h {
    font-family: var(--wp-font-display);
    font-size: 13px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: var(--wp-text-muted);
  }
  .new,
  .prim,
  .ghost {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    height: 28px;
    padding: 0 10px;
    border-radius: var(--wp-r-sm);
    font-size: 12px;
    cursor: pointer;
    border: 1px solid var(--wp-border);
    background: var(--wp-card);
    color: var(--wp-text-muted);
  }
  .new:hover {
    color: var(--wp-text);
    background: var(--wp-elevated);
  }
  .prim {
    background: var(--wp-accent);
    color: var(--wp-on-accent);
    border-color: transparent;
  }
  .prim:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }
  .composer {
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: 8px;
    border: 1px solid var(--wp-border);
    border-radius: var(--wp-r-md);
    background: var(--wp-surface);
  }
  .crow {
    display: flex;
    justify-content: flex-end;
    gap: 6px;
  }
  .in {
    padding: 6px 8px;
    border-radius: var(--wp-r-sm);
    border: 1px solid var(--wp-border);
    background: var(--wp-canvas);
    color: var(--wp-text);
    font-size: 13px;
    width: 100%;
  }
  .ta {
    resize: vertical;
    font-family: var(--wp-font-sans);
  }
  .searchbar {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 0 8px;
    height: 32px;
    border: 1px solid var(--wp-border);
    border-radius: var(--wp-r-pill);
    background: var(--wp-surface);
    color: var(--wp-text-subtle);
  }
  .sin {
    border: none;
    background: none;
    color: var(--wp-text);
    font-size: 13px;
    width: 100%;
    outline: none;
  }
  .reslabel {
    font-size: 11px;
    color: var(--wp-text-subtle);
    padding: 2px 4px;
  }
  .threadrow {
    display: flex;
    flex-direction: column;
    gap: 2px;
    text-align: left;
    padding: 8px;
    border: 1px solid var(--wp-border);
    border-radius: var(--wp-r-md);
    background: var(--wp-card);
    cursor: pointer;
    color: var(--wp-text);
  }
  .threadrow:hover {
    background: var(--wp-elevated);
  }
  .threadrow.active {
    border-color: var(--wp-accent);
    background: color-mix(in srgb, var(--wp-accent) 8%, transparent);
  }
  .tr-top {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }
  .tr-id {
    font-family: var(--wp-font-mono);
    font-size: 11px;
    color: var(--wp-text-subtle);
  }
  .tr-count {
    font-family: var(--wp-font-mono);
    font-size: 11px;
    min-width: 18px;
    text-align: center;
    padding: 0 5px;
    border-radius: var(--wp-r-pill);
    background: var(--wp-surface);
    border: 1px solid var(--wp-border);
    color: var(--wp-text-subtle);
  }
  .tr-title {
    font-size: 13px;
    font-weight: 500;
  }
  .tr-meta,
  .tr-author {
    font-size: 11px;
    color: var(--wp-text-subtle);
  }
  .tr-snip {
    font-size: 12px;
    color: var(--wp-text-muted);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .empty,
  .pick {
    color: var(--wp-text-subtle);
    font-size: 13px;
    padding: 12px 6px;
  }
  .pick {
    display: flex;
    height: 100%;
    align-items: center;
    justify-content: center;
  }
  .thread {
    display: flex;
    flex-direction: column;
    min-height: 0;
  }
  .th-head {
    display: flex;
    align-items: baseline;
    gap: 8px;
    padding-bottom: 10px;
    border-bottom: 1px solid var(--wp-border);
  }
  .th-head h2 {
    font-family: var(--wp-font-display);
    font-size: 18px;
    font-weight: 600;
  }
  .th-id {
    font-family: var(--wp-font-mono);
    font-size: 12px;
    color: var(--wp-text-subtle);
  }
  .feed {
    flex: 1;
    overflow-y: auto;
    padding: 12px 2px;
    display: flex;
    flex-direction: column;
    gap: 12px;
  }
  .post {
    position: relative;
    display: flex;
    gap: 8px;
    margin-left: calc(var(--indent) * 22px);
  }
  .thread-line {
    position: absolute;
    left: -12px;
    top: -8px;
    bottom: 12px;
    width: 2px;
    background: var(--wp-border);
    border-radius: 2px;
  }
  .bubble {
    flex: 1;
    min-width: 0;
    border: 1px solid var(--wp-border);
    border-radius: var(--wp-r-md);
    background: var(--wp-surface);
    padding: 8px 10px;
  }
  .p-head {
    display: flex;
    align-items: center;
    gap: 6px;
  }
  .p-author {
    font-size: 13px;
    font-weight: 600;
  }
  .p-id {
    font-family: var(--wp-font-mono);
    font-size: 11px;
    color: var(--wp-text-subtle);
  }
  .p-time {
    font-family: var(--wp-font-mono);
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
  .post:hover .p-actions {
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
    margin: 4px 0;
  }
  .p-body {
    font-size: 13px;
    line-height: 1.5;
  }
  .replybar {
    border-top: 1px solid var(--wp-border);
    padding-top: 10px;
  }
  .replying {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    font-size: 11px;
    color: var(--wp-accent);
    margin-bottom: 6px;
  }
  .rx {
    display: inline-flex;
    border: none;
    background: none;
    color: var(--wp-text-subtle);
    cursor: pointer;
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
  .err {
    font-size: 12px;
    color: var(--wp-error);
    padding: 4px 0;
  }
</style>
