<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { get } from 'svelte/store';
  import { MessageSquarePlus, Search, Send, X } from 'lucide-svelte';
  import ForumPost from './ForumPost.svelte';
  import { api, subscribeChanges } from '$lib/api';
  import { currentProject, identities, definitions } from '$lib/stores/board';
  import { labelColor } from '$lib/utils';
  import type { ForumMatch, ForumPost as Post, ForumThread, ForumThreadSummary } from '$lib/types';

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
  let ntLabels = $state<string[]>([]);
  let replyTo = $state<string | null>(null);
  let replyBody = $state('');
  let sending = $state(false); // in-flight guard for createThread / sendReply

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

  function findPost(root: Post | undefined, id: string): Post | undefined {
    if (!root) return undefined;
    if (root.id === id) return root;
    for (const r of root.replies ?? []) {
      const hit = findPost(r, id);
      if (hit) return hit;
    }
    return undefined;
  }
  // The person you're replying to, by name - never the raw post ID.
  let replyTargetName = $derived(
    replyTo && thread && replyTo !== thread.id
      ? displayName(findPost(thread.root, replyTo)?.author ?? '')
      : null
  );

  async function loadThreads() {
    try {
      threads = await api.forumThreads(proj());
      error = null;
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    }
  }

  async function openThread(id: string) {
    const switching = id !== selectedId;
    selectedId = id;
    results = null;
    // Only reset the reply target when actually switching threads, so a background
    // WS refresh doesn't clobber an in-progress reply aimed at a nested post.
    if (switching) replyTo = id;
    try {
      const t = await api.forumThread(id, proj());
      if (id !== selectedId) return; // drop a stale response
      thread = t;
      // If the reply was aimed at a post that has since been deleted (locally or by
      // another user, via a WS refresh), re-aim it at the thread root - otherwise
      // the composer says "reply to thread" but sends to a gone id and 400s.
      if (replyTo && !findPost(t.root, replyTo)) replyTo = t.id;
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    }
  }

  async function refresh() {
    await loadThreads();
    if (selectedId) {
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

  function toggleNewLabel(name: string) {
    ntLabels = ntLabels.includes(name) ? ntLabels.filter((l) => l !== name) : [...ntLabels, name];
  }

  async function createThread() {
    const title = ntTitle.trim();
    if (!title || sending) return;
    sending = true;
    try {
      const t = await api.forumPost(
        { title, body: ntBody.trim() || undefined, labels: ntLabels },
        proj()
      );
      ntTitle = '';
      ntBody = '';
      ntLabels = [];
      composingThread = false;
      await loadThreads();
      await openThread(t.id);
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    } finally {
      sending = false;
    }
  }

  function startReply(id: string) {
    replyTo = id;
    replyBody = '';
  }

  async function sendReply() {
    const body = replyBody.trim();
    if (!body || !replyTo || sending) return;
    sending = true;
    try {
      await api.forumReply(replyTo, { body }, proj());
      replyBody = '';
      if (selectedId) await openThread(selectedId);
      await loadThreads();
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    } finally {
      sending = false;
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

  // Reload (and reset selection) whenever the current project changes; also the
  // initial load on mount.
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
        {#if $definitions.labels.length}
          <div class="pool">
            {#each $definitions.labels as label (label.name)}
              <button
                type="button"
                class="pchip"
                class:on={ntLabels.includes(label.name)}
                style="--c:{labelColor(label.name, label.color)}"
                onclick={() => toggleNewLabel(label.name)}
              >
                {label.name}
              </button>
            {/each}
          </div>
        {/if}
        <div class="crow">
          <button class="ghost" onclick={() => (composingThread = false)}>Cancel</button>
          <button class="prim" disabled={!ntTitle.trim() || sending} onclick={createThread}
            >Post thread</button
          >
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
          <div class="tr-meta">{displayName(r.author)}</div>
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
          <div class="tr-title">{t.title}</div>
          <div class="tr-sub">
            <span class="tr-meta">{displayName(t.author)}</span>
            <span class="tr-count">{t.posts}</span>
          </div>
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
      </header>
      <div class="feed wp-scroll">
        <ForumPost post={thread.root} onreply={startReply} ondelete={del} />
      </div>
      <div class="replybar">
        {#if replyTargetName}
          <div class="replying">
            replying to {replyTargetName}
            <button class="rx" aria-label="Reply to thread instead" onclick={() => (replyTo = thread!.id)}>
              <X size={12} />
            </button>
          </div>
        {/if}
        <div class="c-add">
          <textarea
            class="c-input"
            rows="2"
            placeholder={replyTargetName ? `Reply to ${replyTargetName}…` : 'Reply to this thread…'}
            bind:value={replyBody}
            onkeydown={(e) => {
              if (e.key === 'Enter' && (e.metaKey || e.ctrlKey)) sendReply();
            }}
          ></textarea>
          <button
            class="c-send"
            aria-label="Send reply"
            disabled={!replyBody.trim() || sending}
            onclick={sendReply}><Send size={15} /></button
          >
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
  .pool {
    display: flex;
    flex-wrap: wrap;
    gap: 5px;
  }
  .pchip {
    display: inline-flex;
    align-items: center;
    height: 22px;
    padding: 0 9px;
    border-radius: var(--wp-r-pill);
    border: 1px solid color-mix(in srgb, var(--c) 55%, transparent);
    background: none;
    color: var(--wp-text-muted);
    font-size: 11px;
    cursor: pointer;
    transition: all var(--wp-fast) var(--wp-ease);
  }
  .pchip::before {
    content: '';
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--c);
    margin-right: 6px;
  }
  .pchip.on {
    background: color-mix(in srgb, var(--c) 16%, transparent);
    border-color: var(--c);
    color: var(--wp-text);
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
    gap: 4px;
    text-align: left;
    padding: 9px 10px;
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
  .tr-title {
    font-size: 13px;
    font-weight: 500;
    line-height: 1.3;
  }
  .tr-sub {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }
  .tr-meta {
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
    padding-bottom: 10px;
    border-bottom: 1px solid var(--wp-border);
  }
  .th-head h2 {
    font-family: var(--wp-font-display);
    font-size: 18px;
    font-weight: 600;
  }
  .feed {
    flex: 1;
    overflow-y: auto;
    padding: 14px 2px;
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
