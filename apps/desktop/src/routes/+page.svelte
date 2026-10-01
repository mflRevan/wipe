<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import {
    RefreshCw,
    Settings,
    History,
    RotateCcw,
    WifiOff,
    LayoutGrid,
    MessagesSquare,
    Hash
  } from 'lucide-svelte';
  import {
    board,
    boardError,
    health,
    healthError,
    currentProject,
    rewinding,
    rewindCommit,
    loading,
    forumUnread,
    bootstrap,
    checkHealth,
    loadBoard,
    loadProjects,
    reloadProject,
    returnToNow,
    setForumView,
    stopLiveUpdates
  } from '$lib/stores/board';
  import { api, getApiBase } from '$lib/api';
  import SearchBar from '$lib/components/SearchBar.svelte';
  import QuickJump from '$lib/components/QuickJump.svelte';
  import NotifyMenu from '$lib/components/NotifyMenu.svelte';
  import Toasts from '$lib/components/Toasts.svelte';
  import { openRequest } from '$lib/stores/notify';
  import { formatDate } from '$lib/utils';
  import Board from '$lib/components/Board.svelte';
  import Forum from '$lib/components/Forum.svelte';
  import ProjectSwitcher from '$lib/components/ProjectSwitcher.svelte';
  import Lightbox from '$lib/components/Lightbox.svelte';
  import TicketModal from '$lib/components/TicketModal.svelte';
  import NewTicketDialog from '$lib/components/NewTicketDialog.svelte';
  import GitGraph from '$lib/components/GitGraph.svelte';
  import BoardSettings from '$lib/components/BoardSettings.svelte';
  import ThemeToggle from '$lib/components/ThemeToggle.svelte';
  import Button from '$lib/components/ui/Button.svelte';
  import type { Ticket } from '$lib/types';

  let modalTicketId = $state<string | null>(null);
  let newTicketOpen = $state(false);
  let newTicketList = $state('');
  let newTicketName = $state('');

  let settingsOpen = $state(false);
  let historyOpen = $state(false);
  // Restore the last-open view across a page refresh (paired with the persisted
  // board in the store), so a reload stays where you were.
  const VIEW_KEY = 'wipe:view';
  function readView(): 'board' | 'forum' {
    try {
      return typeof localStorage !== 'undefined' && localStorage.getItem(VIEW_KEY) === 'forum'
        ? 'forum'
        : 'board';
    } catch {
      return 'board';
    }
  }
  let view = $state<'board' | 'forum'>(readView());
  $effect(() => {
    try {
      localStorage.setItem(VIEW_KEY, view);
    } catch {
      /* best-effort */
    }
  });

  // Keep the store informed of whether the forum is open, so it can clear the
  // unread indicator while you're looking at it.
  $effect(() => setForumView(view === 'forum'));

  onMount(() => {
    void bootstrap();
  });
  onDestroy(() => stopLiveUpdates());

  async function selectProject(path: string) {
    currentProject.set(path);
    await reloadProject();
  }

  function openTicket(t: Ticket) {
    modalTicketId = t.id;
  }
  function addToList(listId: string, listName: string) {
    newTicketList = listId;
    newTicketName = listName;
    newTicketOpen = true;
  }

  // A clicked notification/toast: open the ticket, or switch to the forum
  // (which opens the thread itself).
  $effect(() => {
    const r = $openRequest;
    if (!r) return;
    if (r.ticket) {
      view = 'board';
      modalTicketId = r.ticket;
      openRequest.set(null);
    } else if (r.thread) {
      view = 'forum';
    }
  });

  // Legacy decimal boards (T-23) can be converted to hex ids (T-017) once.
  let translating = $state(false);
  async function translateIds() {
    if (
      !confirm(
        'Convert this board to the new ticket ids?' +
          ' T-23 becomes T-017 (fixed-width hex): ticket files are renamed and every reference is rewritten.' +
          ' Old ids keep working as aliases in search, the CLI and commit links.' +
          ' Commit or merge other branches first, and have collaborators pull right after.'
      )
    )
      return;
    translating = true;
    try {
      await api.translateIds($currentProject ?? undefined);
      await loadBoard();
    } catch (e) {
      boardError.set(e instanceof Error ? e.message : String(e));
    } finally {
      translating = false;
    }
  }

  // In the forum, Ctrl/Cmd+F jumps to the forum's own search box.
  function onKey(e: KeyboardEvent) {
    if (view !== 'forum' || !(e.ctrlKey || e.metaKey) || e.key.toLowerCase() !== 'f') return;
    const box = document.querySelector<HTMLInputElement>('.forum input[type="text"], .forum input[type="search"], .forum input');
    if (box) {
      e.preventDefault();
      box.focus();
      box.select();
    }
  }

  async function refresh() {
    // If we were offline (the "Retry connection" path), a plain reload would show
    // a board that then never updates: bootstrap() is what starts the 0.5s poll and
    // the change WebSocket, and it was skipped while the daemon was down. Re-run it
    // so a recovered connection resumes live updates instead of freezing.
    const wasOffline = !$health;
    if (!(await checkHealth())) return;
    if (wasOffline) {
      await bootstrap();
      return;
    }
    await loadProjects();
    await loadBoard();
  }
</script>

<svelte:window onkeydown={onKey} />

<div class="app">
  <header class="topbar">
    <div class="brand">
      <span class="wordmark">wipe</span>
    </div>
    <div class="sep"></div>
    <ProjectSwitcher onselect={selectProject} />

    <div class="viewtabs">
      <button class:on={view === 'board'} onclick={() => (view = 'board')}>
        <LayoutGrid size={14} /> Board
      </button>
      <button class:on={view === 'forum'} onclick={() => (view = 'forum')}>
        <MessagesSquare size={14} /> Forum
        {#if $forumUnread && view !== 'forum'}<span class="unread" title="New forum activity"></span>{/if}
      </button>
    </div>

    <div class="right">
      {#if $board && $board.ids !== 'hex' && !$rewinding}
        <button
          class="ib wide"
          disabled={translating}
          title="This board uses the old ticket ids (T-23). Convert to the new fixed-width hex ids (T-017)."
          onclick={translateIds}
        >
          <Hash size={14} />
          <span>{translating ? 'Converting…' : 'New ids'}</span>
        </button>
      {/if}
      {#if $health}
        <span class="status ok" title="daemon v{$health.version}">
          <span class="dot"></span>
          <span class="stxt">v{$health.version}</span>
        </span>
      {:else}
        <span class="status off"><span class="dot"></span><span class="stxt">offline</span></span>
      {/if}

      <NotifyMenu />
      <button class="ib" aria-label="History" title="History" onclick={() => (historyOpen = true)}>
        <History size={16} />
      </button>
      <button class="ib" aria-label="Refresh" title="Refresh" onclick={refresh}>
        <RefreshCw size={16} class={$loading ? 'spin' : ''} />
      </button>
      <ThemeToggle />
      <button class="ib" aria-label="Settings" title="Board settings" onclick={() => (settingsOpen = true)}>
        <Settings size={16} />
      </button>
    </div>
  </header>

  <main class="main">
    {#if !$health}
      <div class="offline">
        <div class="offcard">
          <div class="officon"><WifiOff size={22} /></div>
          <h2>Can't reach the wipe daemon</h2>
          <p>
            Start it from your project with
            <code>wipe serve</code>
            then retry. Expecting the API at
            <span class="mono">{getApiBase()}</span>.
          </p>
          {#if $healthError}<p class="dim">({$healthError})</p>{/if}
          <div class="offactions">
            <Button variant="primary" onclick={refresh}>Retry connection</Button>
            <Button onclick={() => (settingsOpen = true)}>Change API URL</Button>
          </div>
        </div>
      </div>
    {:else if view === 'forum'}
      <div class="boardwrap"><Forum /></div>
    {:else}
      {#if $rewinding && $rewindCommit}
        <div class="banner rewind">
          <span class="rw-tag"><History size={13} /> Viewing snapshot</span>
          <span class="rw-hash">{$rewindCommit.short}</span>
          <span class="rw-subj">{$rewindCommit.subject}</span>
          <span class="rw-meta">· {$rewindCommit.author_name} · {formatDate($rewindCommit.date)}</span>
          <button class="rw-now" onclick={returnToNow}><RotateCcw size={13} /> Return to now</button>
        </div>
      {/if}

      {#if $boardError}
        <div class="banner err">{$boardError}</div>
      {/if}

      <SearchBar enabled={view === 'board' && !!$health} />

      <div class="boardwrap">
        {#if $board}
          <Board onopen={openTicket} onadd={addToList} />
        {:else if $loading}
          <div class="loading">Loading board…</div>
        {/if}
      </div>
    {/if}
  </main>
</div>

<Toasts />
<QuickJump enabled={view === 'board' && !!$health && !$rewinding} onopen={openTicket} />
<TicketModal bind:ticketId={modalTicketId} />
<NewTicketDialog bind:open={newTicketOpen} listId={newTicketList} listName={newTicketName} />
<GitGraph bind:open={historyOpen} />
<BoardSettings bind:open={settingsOpen} />
<Lightbox />

<style>
  .app {
    display: flex;
    flex-direction: column;
    height: 100vh;
    /* Mobile browsers: size to the visible viewport, not behind the URL bar. */
    height: 100dvh;
    background: var(--wp-canvas);
  }
  .topbar {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 10px 16px;
    border-bottom: 1px solid var(--wp-border);
    background: var(--wp-canvas);
  }
  .wordmark {
    font-family: var(--wp-font-display);
    font-size: 18px;
    font-weight: 600;
    letter-spacing: -0.02em;
    color: var(--wp-text);
  }
  .sep {
    width: 1px;
    height: 20px;
    background: var(--wp-border);
  }
  .viewtabs {
    display: inline-flex;
    gap: 2px;
    padding: 2px;
    border-radius: var(--wp-r-md);
    background: var(--wp-surface);
    border: 1px solid var(--wp-border);
  }
  .viewtabs button {
    position: relative;
    display: inline-flex;
    align-items: center;
    gap: 5px;
    height: 26px;
    padding: 0 10px;
    border: none;
    border-radius: var(--wp-r-sm);
    background: none;
    color: var(--wp-text-muted);
    font-size: 12px;
    font-weight: 500;
    cursor: pointer;
  }
  .viewtabs .unread {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: var(--wp-accent);
    box-shadow: 0 0 0 2px var(--wp-surface);
  }
  .viewtabs button:hover {
    color: var(--wp-text);
  }
  .viewtabs button.on {
    background: var(--wp-card);
    color: var(--wp-text);
    box-shadow: var(--wp-shadow);
  }
  .right {
    margin-left: auto;
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .status {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    font-family: var(--wp-font-mono);
    font-size: 12px;
    color: var(--wp-text-muted);
  }
  .status .dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
  }
  .status.ok .dot {
    background: #7e9b7a;
  }
  .status.off .dot {
    background: var(--wp-error);
  }
  .ib {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    height: 32px;
    width: 32px;
    border-radius: var(--wp-r-sm);
    border: 1px solid var(--wp-border);
    background: var(--wp-card);
    color: var(--wp-text-muted);
    cursor: pointer;
    transition: all var(--wp-fast) var(--wp-ease);
  }
  .ib:hover {
    background: var(--wp-elevated);
    color: var(--wp-text);
  }
  .ib.wide {
    width: auto;
    gap: 6px;
    padding: 0 10px;
    font-size: 12px;
    font-weight: 500;
    border-color: color-mix(in srgb, var(--wp-accent) 55%, var(--wp-border));
    color: var(--wp-accent);
  }
  :global(.spin) {
    animation: spin 0.9s linear infinite;
  }
  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }
  .main {
    flex: 1;
    display: flex;
    flex-direction: column;
    min-height: 0;
    padding: 14px 16px 16px;
    gap: 12px;
  }
  .boardwrap {
    flex: 1;
    min-height: 0;
  }
  .loading {
    display: flex;
    height: 100%;
    align-items: center;
    justify-content: center;
    color: var(--wp-text-muted);
  }
  .banner.err {
    padding: 8px 12px;
    border-radius: var(--wp-r-md);
    border: 1px solid color-mix(in srgb, var(--wp-error) 40%, transparent);
    background: color-mix(in srgb, var(--wp-error) 12%, transparent);
    color: var(--wp-error);
    font-size: 13px;
  }
  .banner.rewind {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 8px 12px;
    border-radius: var(--wp-r-md);
    border: 1px solid var(--wp-accent);
    background: color-mix(in srgb, var(--wp-accent) 10%, transparent);
    font-size: 13px;
    min-width: 0;
  }
  .rw-tag {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    flex: none;
    font-weight: 500;
    color: var(--wp-accent);
  }
  .rw-hash {
    font-family: var(--wp-font-mono);
    color: var(--wp-accent);
    flex: none;
  }
  .rw-subj {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    color: var(--wp-text);
  }
  .rw-meta {
    color: var(--wp-text-subtle);
    white-space: nowrap;
    flex: none;
  }
  .rw-now {
    margin-left: auto;
    display: inline-flex;
    align-items: center;
    gap: 4px;
    flex: none;
    padding: 4px 10px;
    border-radius: var(--wp-r-sm);
    border: 1px solid var(--wp-border);
    background: var(--wp-card);
    color: var(--wp-text);
    font-size: 12px;
    cursor: pointer;
  }
  .rw-now:hover {
    background: var(--wp-elevated);
  }
  .offline {
    flex: 1;
    display: flex;
    align-items: center;
    justify-content: center;
  }
  .offcard {
    max-width: 420px;
    text-align: center;
    padding: 28px;
    background: var(--wp-card);
    border: 1px solid var(--wp-border);
    border-radius: var(--wp-r-lg);
    box-shadow: var(--wp-shadow);
  }
  .officon {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 48px;
    height: 48px;
    border-radius: var(--wp-r-pill);
    background: color-mix(in srgb, var(--wp-error) 12%, transparent);
    color: var(--wp-error);
    margin-bottom: 14px;
  }
  .offcard h2 {
    font-family: var(--wp-font-display);
    font-size: 17px;
    font-weight: 600;
    margin-bottom: 8px;
  }
  .offcard p {
    font-size: 13px;
    color: var(--wp-text-muted);
    margin: 0 0 8px;
    line-height: 1.5;
  }
  .offcard code,
  .mono {
    font-family: var(--wp-font-mono);
    font-size: 12px;
  }
  .offcard code {
    background: var(--wp-surface);
    border: 1px solid var(--wp-border);
    border-radius: var(--wp-r-sm);
    padding: 1px 5px;
    color: var(--wp-text);
  }
  .dim {
    color: var(--wp-text-subtle);
    font-size: 12px;
  }
  /* Phones: two header rows - brand, project and actions on top, the
     Board/Forum switch full-width below - and tighter page padding. */
  @media (max-width: 700px) {
    .topbar {
      flex-wrap: wrap;
      gap: 8px 10px;
      padding: 8px 12px;
    }
    .sep,
    .status {
      display: none;
    }
    .topbar :global(> *:nth-child(3)) {
      min-width: 0;
      flex: 1 1 auto;
    }
    .viewtabs {
      order: 10;
      width: 100%;
    }
    .viewtabs button {
      flex: 1;
      justify-content: center;
      height: 34px;
      font-size: 13px;
    }
    .right {
      gap: 6px;
    }
    .ib {
      width: 36px;
      height: 36px;
    }
    .main {
      padding: 10px 0 0;
    }
    .banner.err,
    .banner.rewind {
      margin: 0 12px;
    }
    .banner.rewind {
      flex-wrap: wrap;
    }
  }
  .offactions {
    display: flex;
    justify-content: center;
    gap: 8px;
    margin-top: 16px;
  }
</style>
