<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import {
    board,
    boardError,
    health,
    healthError,
    currentProject,
    rewinding,
    rewindCommit,
    loading,
    bootstrap,
    checkHealth,
    loadBoard,
    loadHistory,
    loadProjects,
    returnToNow,
    stopLiveUpdates
  } from '$lib/stores/board';
  import { getApiBase, setApiBase } from '$lib/api';
  import Board from '$lib/components/Board.svelte';
  import HistoryBar from '$lib/components/HistoryBar.svelte';
  import ProjectSwitcher from '$lib/components/ProjectSwitcher.svelte';
  import TicketDrawer from '$lib/components/TicketDrawer.svelte';
  import NewTicketDialog from '$lib/components/NewTicketDialog.svelte';
  import Dialog from '$lib/components/ui/Dialog.svelte';
  import Input from '$lib/components/ui/Input.svelte';
  import Button from '$lib/components/ui/Button.svelte';
  import type { Ticket } from '$lib/types';
  import { formatDate } from '$lib/utils';
  import { RefreshCw, Settings, Rotate3d } from 'lucide-svelte';

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
    await returnToNow();
    await loadHistory();
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
      await loadHistory();
    }
  }
</script>

<div class="flex h-screen flex-col bg-background">
  <!-- top bar -->
  <header class="flex items-center gap-3 border-b border-border px-5 py-3">
    <div class="flex items-center gap-2.5">
      <div class="flex h-7 w-7 items-center justify-center rounded-lg bg-primary/15">
        <Rotate3d class="h-4 w-4 text-primary" />
      </div>
      <span class="text-base font-semibold tracking-tight">wipe</span>
    </div>

    <div class="mx-1 h-5 w-px bg-border"></div>

    <ProjectSwitcher onselect={selectProject} />

    <div class="ml-auto flex items-center gap-3">
      <!-- health indicator -->
      {#if $health}
        <span class="flex items-center gap-1.5 text-xs text-muted-foreground">
          <span class="h-2 w-2 rounded-full bg-emerald-400 shadow-[0_0_8px] shadow-emerald-400/50"
          ></span>
          <span class="hidden md:inline">daemon v{$health.version}</span>
        </span>
      {:else}
        <span class="flex items-center gap-1.5 text-xs text-muted-foreground">
          <span class="h-2 w-2 rounded-full bg-rose-500"></span>
          <span class="hidden md:inline">offline</span>
        </span>
      {/if}

      <Button
        variant="ghost"
        size="icon"
        class="h-8 w-8"
        aria-label="Refresh"
        onclick={refresh}
      >
        <RefreshCw class="h-4 w-4 {$loading ? 'animate-spin' : ''}" />
      </Button>
      <Button
        variant="ghost"
        size="icon"
        class="h-8 w-8"
        aria-label="Settings"
        onclick={() => {
          apiBaseInput = getApiBase();
          settingsOpen = true;
        }}
      >
        <Settings class="h-4 w-4" />
      </Button>
    </div>
  </header>

  <!-- body -->
  <main class="flex flex-1 flex-col overflow-hidden p-5">
    {#if !$health}
      <!-- daemon unreachable -->
      <div class="flex flex-1 items-center justify-center">
        <div class="max-w-md rounded-2xl border border-border bg-card p-8 text-center shadow-lg">
          <div
            class="mx-auto mb-4 flex h-12 w-12 items-center justify-center rounded-full bg-rose-500/10"
          >
            <span class="h-3 w-3 rounded-full bg-rose-500"></span>
          </div>
          <h2 class="text-lg font-semibold">Can’t reach the wipe daemon</h2>
          <p class="mt-2 text-sm text-muted-foreground">
            Start it from your project with
            <code class="rounded bg-muted px-1.5 py-0.5 font-mono text-foreground">wipe serve</code>
            then retry. Expecting the API at
            <span class="font-mono text-foreground">{getApiBase()}</span>.
          </p>
          {#if $healthError}
            <p class="mt-2 text-xs text-muted-foreground/70">({$healthError})</p>
          {/if}
          <div class="mt-5 flex justify-center gap-2">
            <Button onclick={refresh}>Retry connection</Button>
            <Button
              variant="outline"
              onclick={() => {
                apiBaseInput = getApiBase();
                settingsOpen = true;
              }}>Change API URL</Button
            >
          </div>
        </div>
      </div>
    {:else}
      <!-- rewind banner -->
      {#if $rewinding && $rewindCommit}
        <div
          class="mb-4 flex flex-wrap items-center gap-x-3 gap-y-1 rounded-xl border border-amber-500/30 bg-amber-500/10 px-4 py-2.5 text-sm"
        >
          <span class="font-mono font-semibold text-amber-300">{$rewindCommit.short}</span>
          <span class="text-foreground/90">{$rewindCommit.subject}</span>
          <span class="text-muted-foreground">
            · {$rewindCommit.author_name} · {formatDate($rewindCommit.date)}
          </span>
          <span
            class="ml-1 rounded-full border border-amber-500/30 px-2 py-0.5 text-[11px] font-medium text-amber-300"
          >
            read-only
          </span>
          <div class="ml-auto">
            <Button size="sm" variant="secondary" onclick={returnToNow}>Return to now</Button>
          </div>
        </div>
      {/if}

      <!-- history / time machine -->
      <div class="mb-4">
        <HistoryBar />
      </div>

      {#if $boardError}
        <div
          class="mb-4 rounded-lg border border-destructive/40 bg-destructive/10 px-4 py-2 text-sm text-destructive"
        >
          {$boardError}
        </div>
      {/if}

      <!-- board -->
      <div class="min-h-0 flex-1">
        {#if $board}
          <Board onopen={openTicket} onadd={addToList} />
        {:else if $loading}
          <div class="flex h-full items-center justify-center text-sm text-muted-foreground">
            Loading board…
          </div>
        {/if}
      </div>
    {/if}
  </main>
</div>

<!-- overlays -->
<TicketDrawer bind:ticketId={drawerTicketId} />
<NewTicketDialog bind:open={newTicketOpen} listId={newTicketList} listName={newTicketName} />

<Dialog bind:open={settingsOpen} title="Settings" description="Where the wipe daemon is reachable.">
  <div class="space-y-2">
    <label for="api-base" class="text-xs font-medium text-muted-foreground">API base URL</label>
    <Input id="api-base" bind:value={apiBaseInput} placeholder="http://localhost:6737" />
    <p class="text-xs text-muted-foreground">
      Overrides <code class="font-mono">VITE_WIPE_API</code>. Stored locally in this browser.
    </p>
  </div>
  {#snippet footer()}
    <Button variant="ghost" onclick={() => (settingsOpen = false)}>Cancel</Button>
    <Button onclick={saveSettings}>Save &amp; reconnect</Button>
  {/snippet}
</Dialog>
