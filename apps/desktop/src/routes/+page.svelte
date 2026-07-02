<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { fade, scale } from 'svelte/transition';
  import { RefreshCw, Settings, X, WifiOff } from 'lucide-svelte';
  import {
    board,
    boardError,
    health,
    healthError,
    currentProject,
    rewinding,
    loading,
    bootstrap,
    checkHealth,
    loadBoard,
    loadProjects,
    reloadProject,
    stopLiveUpdates
  } from '$lib/stores/board';
  import { getApiBase, setApiBase } from '$lib/api';
  import Board from '$lib/components/Board.svelte';
  import TimeMachine from '$lib/components/TimeMachine.svelte';
  import ProjectSwitcher from '$lib/components/ProjectSwitcher.svelte';
  import TicketDrawer from '$lib/components/TicketDrawer.svelte';
  import NewTicketDialog from '$lib/components/NewTicketDialog.svelte';
  import ThemeToggle from '$lib/components/ThemeToggle.svelte';
  import Button from '$lib/components/ui/Button.svelte';
  import type { Ticket } from '$lib/types';

  let drawerTicketId = $state<string | null>(null);
  let newTicketOpen = $state(false);
  let newTicketList = $state('');
  let newTicketName = $state('');

  let settingsOpen = $state(false);
  let apiBaseInput = $state('');

  onMount(() => {
    apiBaseInput = getApiBase();
    void bootstrap();
  });
  onDestroy(() => stopLiveUpdates());

  async function selectProject(path: string) {
    currentProject.set(path);
    await reloadProject();
  }

  function openTicket(t: Ticket) {
    drawerTicketId = t.id;
  }
  function addToList(listId: string, listName: string) {
    newTicketList = listId;
    newTicketName = listName;
    newTicketOpen = true;
  }

  async function saveSettings() {
    setApiBase(apiBaseInput);
    settingsOpen = false;
    stopLiveUpdates();
    await bootstrap();
  }

  async function refresh() {
    if (await checkHealth()) {
      await loadProjects();
      await loadBoard();
    }
  }
</script>

<div class="app">
  <header class="topbar">
    <div class="brand">
      <span class="wordmark">wipe</span>
    </div>
    <div class="sep"></div>
    <ProjectSwitcher onselect={selectProject} />

    <div class="right">
      {#if $health}
        <span class="status ok" title="daemon v{$health.version}">
          <span class="dot"></span>
          <span class="stxt">v{$health.version}</span>
        </span>
      {:else}
        <span class="status off"><span class="dot"></span><span class="stxt">offline</span></span>
      {/if}

      <button class="ib" aria-label="Refresh" title="Refresh" onclick={refresh}>
        <RefreshCw size={16} class={$loading ? 'spin' : ''} />
      </button>
      <ThemeToggle />
      <button
        class="ib"
        aria-label="Settings"
        title="Settings"
        onclick={() => {
          apiBaseInput = getApiBase();
          settingsOpen = true;
        }}
      >
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
            <Button
              onclick={() => {
                apiBaseInput = getApiBase();
                settingsOpen = true;
              }}>Change API URL</Button
            >
          </div>
        </div>
      </div>
    {:else}
      <div class="tmwrap"><TimeMachine /></div>

      {#if $boardError}
        <div class="banner err">{$boardError}</div>
      {/if}

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

<TicketDrawer bind:ticketId={drawerTicketId} />
<NewTicketDialog bind:open={newTicketOpen} listId={newTicketList} listName={newTicketName} />

{#if settingsOpen}
  <div class="scrim" transition:fade={{ duration: 160 }} onclick={() => (settingsOpen = false)} role="presentation"></div>
  <div class="modal-wrap">
    <div class="modal" transition:scale={{ duration: 160, start: 0.96 }} role="dialog" aria-modal="true">
      <header class="m-head">
        <h3>Settings</h3>
        <button class="close" aria-label="Close" onclick={() => (settingsOpen = false)}><X size={18} /></button>
      </header>
      <label class="fl" for="api-base">API base URL</label>
      <input id="api-base" class="in" bind:value={apiBaseInput} placeholder="http://localhost:6737" />
      <p class="hint">
        Overrides <code>VITE_WIPE_API</code>. Stored locally in this browser. Leave blank to use the
        serving origin.
      </p>
      <div class="actions">
        <Button variant="ghost" onclick={() => (settingsOpen = false)}>Cancel</Button>
        <Button variant="primary" onclick={saveSettings}>Save &amp; reconnect</Button>
      </div>
    </div>
  </div>
{/if}

<style>
  .app {
    display: flex;
    flex-direction: column;
    height: 100vh;
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
  .tmwrap:empty {
    display: none;
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
  .offactions {
    display: flex;
    justify-content: center;
    gap: 8px;
    margin-top: 16px;
  }

  /* settings modal */
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
    padding-top: 14vh;
    pointer-events: none;
  }
  .modal {
    pointer-events: auto;
    width: min(440px, 92vw);
    background: var(--wp-card);
    border: 1px solid var(--wp-border);
    border-radius: var(--wp-r-lg);
    box-shadow: var(--wp-shadow-lift);
    padding: 18px;
  }
  .m-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: 10px;
  }
  .m-head h3 {
    font-family: var(--wp-font-display);
    font-size: 16px;
    font-weight: 600;
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
    display: block;
    font-size: 12px;
    font-weight: 500;
    color: var(--wp-text-muted);
    margin-bottom: 6px;
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
  .hint {
    font-size: 12px;
    color: var(--wp-text-subtle);
    margin: 8px 0 0;
  }
  .hint code {
    font-family: var(--wp-font-mono);
  }
  .actions {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    margin-top: 16px;
  }
</style>
