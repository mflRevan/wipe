<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { get } from 'svelte/store';
  import { MessageSquarePlus, Search, Send, X, SlidersHorizontal, MessageSquare, Check } from 'lucide-svelte';
  import ForumPost from './ForumPost.svelte';
  import Avatar from './Avatar.svelte';
  import { api, subscribeChanges } from '$lib/api';
  import { currentProject, identities, definitions } from '$lib/stores/board';
  import { labelColor } from '$lib/utils';
  import type { ForumPost as Post, ForumThread, ForumThreadSummary } from '$lib/types';

  let threads = $state<ForumThreadSummary[]>([]);
  let selectedId = $state<string | null>(null);
  let thread = $state<ForumThread | null>(null);
  let error = $state<string | null>(null);

  // --- sidebar width (resizable + remembered) ------------------------------
  const SIDEBAR_KEY = 'wipe.forum.sidebarWidth';
  let sidebarWidth = $state(300);
  onMount(() => {
    try {
      const v = parseInt(localStorage.getItem(SIDEBAR_KEY) ?? '', 10);
      if (Number.isFinite(v)) sidebarWidth = Math.max(240, Math.min(600, v));
    } catch {
      /* storage unavailable */
    }
  });
  function startResize(e: PointerEvent) {
    e.preventDefault();
    const startX = e.clientX;
    const startW = sidebarWidth;
    const onMove = (ev: PointerEvent) => {
      sidebarWidth = Math.max(240, Math.min(600, startW + (ev.clientX - startX)));
    };
    const onUp = () => {
      window.removeEventListener('pointermove', onMove);
      window.removeEventListener('pointerup', onUp);
      try {
        localStorage.setItem(SIDEBAR_KEY, String(Math.round(sidebarWidth)));
      } catch {
        /* ignore */
      }
    };
    window.addEventListener('pointermove', onMove);
    window.addEventListener('pointerup', onUp);
  }

  // --- search + filters ----------------------------------------------------
  let query = $state('');
  // Thread ids whose posts match the text query (null = no text filter).
  let searchThreadIds = $state<Set<string> | null>(null);
  let filtersOpen = $state(false);
  let fIdentities = $state<string[]>([]); // identity ids that must have engaged
  let fOpOnly = $state(false); // ...only as the original poster
  let fLabels = $state<string[]>([]);
  let activeFilters = $derived(fIdentities.length + fLabels.length + (fOpOnly ? 1 : 0));

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
  /** Compact "last activity" label: 5s → "now", then m / h / d, then a date. */
  function relativeTime(iso?: string): string {
    if (!iso) return '';
    const then = new Date(iso).getTime();
    if (Number.isNaN(then)) return '';
    const s = Math.max(0, (Date.now() - then) / 1000);
    if (s < 45) return 'now';
    if (s < 3600) return `${Math.round(s / 60)}m`;
    if (s < 86400) return `${Math.round(s / 3600)}h`;
    if (s < 7 * 86400) return `${Math.round(s / 86400)}d`;
    return new Date(iso).toLocaleDateString(undefined, { month: 'short', day: 'numeric' });
  }
  function labelColorFor(name: string) {
    const def = $definitions.labels.find((l) => l.name === name);
    return labelColor(name, def?.color);
  }
  // Thread whose feed is open — for the header meta row.
  let openSummary = $derived(threads.find((t) => t.id === selectedId));

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
      searchThreadIds = null;
      return;
    }
    try {
      const matches = await api.forumSearch({ q }, proj());
      searchThreadIds = new Set(matches.map((m) => m.thread_id));
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    }
  }

  /** Whether an author string (`Name <email>` or a bare id) refers to identity `id`. */
  function sameIdentity(author: string, id: string): boolean {
    if (author === id) return true;
    const m = author.match(/<([^>]+)>/);
    return m ? m[1] === id : false;
  }
  function toggleFIdentity(id: string) {
    fIdentities = fIdentities.includes(id)
      ? fIdentities.filter((x) => x !== id)
      : [...fIdentities, id];
  }
  function toggleFLabel(name: string) {
    fLabels = fLabels.includes(name) ? fLabels.filter((x) => x !== name) : [...fLabels, name];
  }
  function clearFilters() {
    fIdentities = [];
    fLabels = [];
    fOpOnly = false;
  }

  // The thread list after applying the text search + identity/OP/label filters.
  let filtered = $derived(
    threads.filter((t) => {
      if (searchThreadIds && !searchThreadIds.has(t.id)) return false;
      if (fLabels.length && !fLabels.some((l) => t.labels?.includes(l))) return false;
      if (fIdentities.length) {
        if (fOpOnly) {
          if (!fIdentities.some((id) => sameIdentity(t.author, id))) return false;
        } else {
          const parts = t.participants ?? [t.author, t.last_author];
          if (!fIdentities.some((id) => parts.some((p) => sameIdentity(p, id)))) return false;
        }
      }
      return true;
    })
  );

  /** Time-bucket a thread by how long ago it was last active. */
  function bucketOf(iso?: string): string {
    if (!iso) return 'Earlier';
    const then = new Date(iso).getTime();
    if (Number.isNaN(then)) return 'Earlier';
    const days = (Date.now() - then) / 86400000;
    if (days < 1) return 'Today';
    if (days < 7) return 'This week';
    if (days < 30) return 'This month';
    return new Date(iso).toLocaleDateString(undefined, { month: 'long', year: 'numeric' });
  }

  // Interleave activity-bucket dividers into the (already activity-sorted) list.
  type Row = { kind: 'divider'; label: string } | { kind: 'thread'; t: ForumThreadSummary };
  let sidebarItems = $derived.by<Row[]>(() => {
    const out: Row[] = [];
    let last: string | null = null;
    for (const t of filtered) {
      const b = bucketOf(t.updated ?? t.created);
      if (b !== last) {
        out.push({ kind: 'divider', label: b });
        last = b;
      }
      out.push({ kind: 'thread', t });
    }
    return out;
  });

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
    searchThreadIds = null;
    query = '';
    clearFilters();
    void loadThreads();
  });

  let unsub: (() => void) | null = null;
  onMount(() => {
    unsub = subscribeChanges(() => void refresh());
  });
  onDestroy(() => unsub?.());
</script>

<div class="forum" style="--sw:{sidebarWidth}px">
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

    <div class="searchrow">
      <div class="searchbar">
        <Search size={14} />
        <input
          class="sin"
          placeholder="Search…"
          bind:value={query}
          onkeydown={(e) => e.key === 'Enter' && runSearch()}
          oninput={() => {
            if (!query.trim()) searchThreadIds = null;
          }}
        />
        {#if query}
          <button
            class="sclear"
            aria-label="Clear search"
            onclick={() => {
              query = '';
              searchThreadIds = null;
            }}><X size={12} /></button
          >
        {/if}
      </div>
      <button
        class="filterbtn"
        class:on={filtersOpen || activeFilters > 0}
        title="Filters"
        onclick={() => (filtersOpen = !filtersOpen)}
      >
        <SlidersHorizontal size={14} />
        {#if activeFilters > 0}<span class="fcount">{activeFilters}</span>{/if}
      </button>
    </div>

    {#if filtersOpen}
      <div class="filters">
        <div class="frow">
          <span class="flabel">Engaged</span>
          <label class="oponly"><input type="checkbox" bind:checked={fOpOnly} /> only as OP</label>
        </div>
        <div class="fpool">
          {#each $identities as id (id.id)}
            <button
              class="ichip"
              class:on={fIdentities.includes(id.id)}
              onclick={() => toggleFIdentity(id.id)}
            >
              <Avatar id={id.id} identity={id} size={15} />
              <span class="iname">{id.display_name || id.id}</span>
            </button>
          {/each}
        </div>
        {#if $definitions.labels.length}
          <span class="flabel">Labels</span>
          <div class="fpool">
            {#each $definitions.labels as label (label.name)}
              <button
                class="pchip"
                class:on={fLabels.includes(label.name)}
                style="--c:{labelColor(label.name, label.color)}"
                onclick={() => toggleFLabel(label.name)}>{label.name}</button
              >
            {/each}
          </div>
        {/if}
        {#if activeFilters > 0}
          <button class="clearf" onclick={clearFilters}>Clear filters</button>
        {/if}
      </div>
    {/if}

    {#if sidebarItems.length === 0}
      <div class="empty">
        {threads.length === 0
          ? 'No threads yet. Start one with New.'
          : 'No threads match your filters.'}
      </div>
    {:else}
      {#each sidebarItems as item (item.kind === 'divider' ? 'd:' + item.label : item.t.id)}
        {#if item.kind === 'divider'}
          <div class="seg">{item.label}</div>
        {:else}
          {@const t = item.t}
          <button
            class="threadrow"
            class:active={t.id === selectedId}
            onclick={() => openThread(t.id)}
          >
            <div class="tr-top">
              <span class="tr-title">{t.title}</span>
              <span class="tr-count" title="{t.posts} post{t.posts === 1 ? '' : 's'}">
                <MessageSquare size={11} />{t.posts}
              </span>
            </div>
            {#if t.snippet}<div class="tr-snip">{t.snippet}</div>{/if}
            <div class="tr-sub">
              {#if t.labels?.length}
                <span class="tr-labels">
                  {#each t.labels.slice(0, 2) as l (l)}
                    <span class="tr-chip" style="--c:{labelColorFor(l)}">{l}</span>
                  {/each}
                  {#if t.labels.length > 2}<span class="tr-more">+{t.labels.length - 2}</span>{/if}
                </span>
              {/if}
              <span class="tr-right">
                <span class="tr-meta">{displayName(t.last_author ?? t.author)}</span>
                <span class="tr-time" title="last activity">{relativeTime(t.updated ?? t.created)}</span>
              </span>
            </div>
          </button>
        {/if}
      {/each}
    {/if}
  </aside>

  <div
    class="resizer"
    role="separator"
    aria-orientation="vertical"
    tabindex="-1"
    onpointerdown={startResize}
  ></div>

  <section class="thread">
    {#if error}<div class="err">{error}</div>{/if}
    {#if thread}
      <header class="th-head">
        <h2>{thread.title}</h2>
        <div class="th-meta">
          <span class="thread-id">{thread.id}</span>
          <span class="dot">·</span>
          <span>started by {displayName(thread.root.author)}</span>
          {#if openSummary}
            <span class="dot">·</span>
            <span>{openSummary.posts} post{openSummary.posts === 1 ? '' : 's'}</span>
            <span class="dot">·</span>
            <span>active {relativeTime(openSummary.updated)}</span>
          {/if}
          {#if openSummary?.labels?.length}
            <span class="th-labels">
              {#each openSummary.labels as l (l)}
                <span class="tr-chip" style="--c:{labelColorFor(l)}">{l}</span>
              {/each}
            </span>
          {/if}
        </div>
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
    position: relative;
    display: grid;
    grid-template-columns: var(--sw, 300px) minmax(0, 1fr);
    gap: 14px;
    height: 100%;
    min-height: 0;
  }
  .side {
    display: flex;
    flex-direction: column;
    gap: 8px;
    overflow-y: auto;
    padding-right: 6px;
    border-right: 1px solid var(--wp-border);
  }
  /* Drag handle sitting on the sidebar's right border to resize it. */
  .resizer {
    position: absolute;
    top: 0;
    bottom: 0;
    left: var(--sw, 300px);
    width: 10px;
    transform: translateX(-5px);
    cursor: col-resize;
    z-index: 6;
    background: none;
  }
  .resizer::after {
    content: '';
    position: absolute;
    inset: 0 4px;
    border-radius: var(--wp-r-pill);
    background: transparent;
    transition: background var(--wp-fast) var(--wp-ease);
  }
  .resizer:hover::after {
    background: color-mix(in srgb, var(--wp-accent) 45%, transparent);
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
  .searchrow {
    display: flex;
    align-items: center;
    gap: 6px;
  }
  .searchbar {
    flex: 1;
    min-width: 0;
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
    min-width: 0;
    outline: none;
  }
  .sclear {
    display: inline-flex;
    border: none;
    background: none;
    color: var(--wp-text-subtle);
    cursor: pointer;
    padding: 2px;
  }
  .sclear:hover {
    color: var(--wp-text);
  }
  .filterbtn {
    position: relative;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    height: 32px;
    width: 34px;
    flex: none;
    border-radius: var(--wp-r-sm);
    border: 1px solid var(--wp-border);
    background: var(--wp-surface);
    color: var(--wp-text-subtle);
    cursor: pointer;
    transition: all var(--wp-fast) var(--wp-ease);
  }
  .filterbtn:hover,
  .filterbtn.on {
    color: var(--wp-text);
    border-color: var(--wp-border-strong);
    background: var(--wp-elevated);
  }
  .fcount {
    position: absolute;
    top: -5px;
    right: -5px;
    min-width: 15px;
    height: 15px;
    padding: 0 4px;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    border-radius: var(--wp-r-pill);
    background: var(--wp-accent);
    color: var(--wp-on-accent);
    font-size: 9px;
    font-weight: 700;
  }
  .filters {
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 10px;
    border: 1px solid var(--wp-border);
    border-radius: var(--wp-r-md);
    background: var(--wp-surface);
  }
  .frow {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }
  .flabel {
    font-family: var(--wp-font-display);
    font-size: 10px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: var(--wp-text-muted);
  }
  .oponly {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    font-size: 11px;
    color: var(--wp-text-subtle);
    cursor: pointer;
  }
  .fpool {
    display: flex;
    flex-wrap: wrap;
    gap: 5px;
  }
  .ichip {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    max-width: 100%;
    height: 24px;
    padding: 0 9px 0 4px;
    border-radius: var(--wp-r-pill);
    border: 1px solid var(--wp-border);
    background: var(--wp-card);
    color: var(--wp-text-muted);
    font-size: 11px;
    cursor: pointer;
    transition: all var(--wp-fast) var(--wp-ease);
  }
  .ichip .iname {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .ichip.on {
    border-color: var(--wp-accent);
    background: color-mix(in srgb, var(--wp-accent) 14%, transparent);
    color: var(--wp-text);
  }
  .clearf {
    align-self: flex-start;
    border: none;
    background: none;
    color: var(--wp-text-subtle);
    font-size: 11px;
    cursor: pointer;
    padding: 2px 0;
  }
  .clearf:hover {
    color: var(--wp-accent);
  }
  /* Minimalistic activity-bucket divider. */
  .seg {
    display: flex;
    align-items: center;
    gap: 8px;
    margin: 8px 2px 2px;
    font-family: var(--wp-font-display);
    font-size: 10px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.07em;
    color: var(--wp-text-subtle);
  }
  .seg::after {
    content: '';
    flex: 1;
    height: 1px;
    background: var(--wp-border);
  }
  .seg:first-child {
    margin-top: 0;
  }
  .threadrow {
    position: relative;
    display: flex;
    flex-direction: column;
    gap: 4px;
    text-align: left;
    padding: 8px 10px 8px 12px;
    border: 1px solid var(--wp-border);
    border-radius: var(--wp-r-md);
    background: var(--wp-card);
    cursor: pointer;
    color: var(--wp-text);
    transition:
      background var(--wp-fast) var(--wp-ease),
      border-color var(--wp-fast) var(--wp-ease);
  }
  .threadrow:hover {
    background: var(--wp-elevated);
    border-color: var(--wp-border-strong);
  }
  /* Active thread: an accent spine on the left + tinted surface. */
  .threadrow.active {
    border-color: color-mix(in srgb, var(--wp-accent) 55%, transparent);
    background: color-mix(in srgb, var(--wp-accent) 9%, var(--wp-card));
  }
  .threadrow.active::before {
    content: '';
    position: absolute;
    left: 0;
    top: 8px;
    bottom: 8px;
    width: 3px;
    border-radius: 0 var(--wp-r-pill) var(--wp-r-pill) 0;
    background: var(--wp-accent);
  }
  .tr-top {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
  }
  .tr-title {
    flex: 1;
    min-width: 0;
    font-family: var(--wp-font-display);
    font-size: 13.5px;
    font-weight: 600;
    line-height: 1.3;
    letter-spacing: -0.005em;
    /* single line - the sidebar stays dense so more threads fit vertically */
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .tr-sub {
    display: flex;
    align-items: center;
    gap: 8px;
    min-width: 0;
  }
  .tr-right {
    display: flex;
    align-items: center;
    gap: 6px;
    margin-left: auto;
    min-width: 0;
  }
  .tr-meta {
    font-size: 11px;
    color: var(--wp-text-subtle);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .tr-time {
    font-family: var(--wp-font-mono);
    font-size: 10.5px;
    color: var(--wp-text-subtle);
    flex: none;
  }
  /* Post counter: distinct hue + icon so it reads as a "conversation size" badge. */
  .tr-count {
    display: inline-flex;
    align-items: center;
    gap: 3px;
    font-family: var(--wp-font-mono);
    font-size: 11px;
    height: 18px;
    padding: 0 7px;
    border-radius: var(--wp-r-pill);
    background: color-mix(in srgb, var(--wp-focus) 15%, transparent);
    color: var(--wp-focus);
    flex: none;
  }
  .tr-snip {
    font-size: 12px;
    line-height: 1.4;
    color: var(--wp-text-muted);
    /* exactly one line, truncated with an ellipsis sized to the sidebar width */
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .tr-labels,
  .th-labels {
    display: flex;
    flex-wrap: nowrap;
    gap: 4px;
    min-width: 0;
    overflow: hidden;
  }
  .tr-more {
    font-size: 10px;
    color: var(--wp-text-subtle);
    align-self: center;
  }
  .tr-chip {
    display: inline-flex;
    align-items: center;
    height: 17px;
    padding: 0 7px 0 6px;
    border-radius: var(--wp-r-pill);
    font-size: 10px;
    font-weight: 500;
    color: color-mix(in srgb, var(--c) 82%, var(--wp-text));
    background: color-mix(in srgb, var(--c) 15%, transparent);
    border: 1px solid color-mix(in srgb, var(--c) 35%, transparent);
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
    padding-bottom: 12px;
    border-bottom: 1px solid var(--wp-border);
  }
  .th-head h2 {
    font-family: var(--wp-font-display);
    font-size: 20px;
    font-weight: 700;
    letter-spacing: -0.01em;
    line-height: 1.25;
  }
  .th-meta {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 6px;
    margin-top: 6px;
    font-size: 11.5px;
    color: var(--wp-text-subtle);
  }
  .th-meta .thread-id {
    font-family: var(--wp-font-mono);
    color: var(--wp-text-muted);
  }
  .th-meta .dot {
    opacity: 0.5;
  }
  .th-meta .th-labels {
    margin-left: 2px;
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
