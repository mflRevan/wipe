<script lang="ts">
  // The navbar bell: notification permission, board-/forum-wide watches, and an
  // overview of everything watched on this board.
  import { Bell, BellRing, BellOff } from 'lucide-svelte';
  import Popover from './ui/Popover.svelte';
  import { watches, toggleWatch, permission, requestPermission } from '$lib/stores/notify';

  let count = $derived(
    ($watches.board ? 1 : 0) +
      ($watches.forum ? 1 : 0) +
      $watches.lists.length +
      $watches.tickets.length +
      $watches.threads.length
  );
</script>

<Popover align="end" width="300px">
  {#snippet trigger({ toggle })}
    <button
      class="ib"
      class:on={count > 0}
      aria-label="Notifications"
      title="Notifications and watches"
      onclick={() => {
        void requestPermission();
        toggle();
      }}
    >
      {#if count > 0}<BellRing size={16} />{:else}<Bell size={16} />{/if}
      {#if count > 0}<span class="badge">{count}</span>{/if}
    </button>
  {/snippet}
  {#snippet children()}
    <div class="menu">
      <div class="head">Notifications</div>
      {#if $permission === 'granted'}
        <p class="state ok">System notifications are on while this tab is in the background.</p>
      {:else if $permission === 'denied'}
        <p class="state warn">
          <BellOff size={13} /> Blocked by the browser - allow notifications for this site in the
          address bar to get them outside the tab. In-app alerts still work.
        </p>
      {:else if $permission === 'unsupported'}
        <p class="state warn">
          System notifications need https or localhost; on this address you'll get in-app alerts.
        </p>
      {:else}
        <button class="enable" onclick={() => void requestPermission()}>Enable system notifications</button>
      {/if}

      <label class="row">
        <input type="checkbox" checked={$watches.board} onchange={() => toggleWatch('board')} />
        <span>Everything on this board</span>
      </label>
      <label class="row">
        <input type="checkbox" checked={$watches.forum} onchange={() => toggleWatch('forum')} />
        <span>Every forum thread</span>
      </label>
      <p class="hint">
        Or watch single items with the bell on a list's menu, a ticket, or a forum thread.
        {#if $watches.lists.length + $watches.tickets.length + $watches.threads.length}
          Watching {$watches.lists.length} list(s), {$watches.tickets.length} ticket(s),
          {$watches.threads.length} thread(s).
        {/if}
        Your own changes never notify you.
      </p>
    </div>
  {/snippet}
</Popover>

<style>
  .ib {
    position: relative;
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
  }
  .ib:hover {
    background: var(--wp-elevated);
    color: var(--wp-text);
  }
  .ib.on {
    color: var(--wp-accent);
  }
  .badge {
    position: absolute;
    top: -5px;
    right: -5px;
    min-width: 16px;
    height: 16px;
    padding: 0 4px;
    border-radius: 8px;
    background: var(--wp-accent);
    color: var(--wp-on-accent, #fff);
    font-size: 10px;
    font-weight: 700;
    line-height: 16px;
  }
  .menu {
    display: flex;
    flex-direction: column;
    gap: 10px;
    padding: 6px;
  }
  .head {
    font-family: var(--wp-font-display);
    font-size: 11px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: var(--wp-text-muted);
  }
  .state {
    margin: 0;
    font-size: 12px;
    line-height: 1.45;
  }
  .state.ok {
    color: #7e9b7a;
  }
  .state.warn {
    color: var(--wp-text-muted);
  }
  .enable {
    height: 32px;
    border-radius: var(--wp-r-sm);
    border: none;
    background: var(--wp-accent);
    color: var(--wp-on-accent, #fff);
    font-weight: 600;
    cursor: pointer;
  }
  .row {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 13px;
    color: var(--wp-text);
    cursor: pointer;
  }
  .hint {
    margin: 0;
    font-size: 12px;
    line-height: 1.45;
    color: var(--wp-text-subtle);
  }
</style>
