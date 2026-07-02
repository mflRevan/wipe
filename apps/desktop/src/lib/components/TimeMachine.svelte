<script lang="ts">
  import { History, RotateCcw } from 'lucide-svelte';
  import { history, rewindCommit, enterRewind, returnToNow } from '$lib/stores/board';
  import { formatDate } from '$lib/utils';
  import type { CommitInfo } from '$lib/types';

  // Slider: index 0 = now (live). index i (1-based) maps to history[i-1].
  let sliderIndex = $derived(
    $rewindCommit ? $history.findIndex((c) => c.hash === $rewindCommit?.hash) + 1 : 0
  );

  async function onInput(e: Event) {
    const idx = Number((e.target as HTMLInputElement).value);
    if (idx === 0) {
      if ($rewindCommit) await returnToNow();
      return;
    }
    const commit: CommitInfo | undefined = $history[idx - 1];
    if (commit && commit.hash !== $rewindCommit?.hash) await enterRewind(commit);
  }
</script>

{#if $history.length > 0}
  <div class="tm" class:active={!!$rewindCommit}>
    <span class="icon"><History size={15} /></span>
    <input
      class="slider"
      type="range"
      min="0"
      max={$history.length}
      value={sliderIndex}
      step="1"
      aria-label="Time machine — rewind board history"
      oninput={onInput}
    />
    <div class="readout">
      {#if $rewindCommit}
        <span class="hash">{$rewindCommit.short}</span>
        <span class="subject">{$rewindCommit.subject}</span>
        <span class="meta">· {$rewindCommit.author_name} · {formatDate($rewindCommit.date)}</span>
        <button class="now" onclick={returnToNow}>
          <RotateCcw size={12} /> Return to now
        </button>
      {:else}
        <span class="live"><span class="dot"></span> Live · {$history.length} commits</span>
      {/if}
    </div>
  </div>
{/if}

<style>
  .tm {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 8px 12px;
    background: var(--wp-surface);
    border: 1px solid var(--wp-border);
    border-radius: var(--wp-r-lg);
  }
  .tm.active {
    border-color: var(--wp-accent);
  }
  .icon {
    color: var(--wp-text-muted);
    flex: none;
    display: inline-flex;
  }
  .slider {
    flex: none;
    width: 240px;
    accent-color: var(--wp-accent);
    cursor: pointer;
  }
  .readout {
    display: flex;
    align-items: center;
    gap: 8px;
    min-width: 0;
    flex: 1;
    font-size: 12px;
  }
  .hash {
    font-family: var(--wp-font-mono);
    font-weight: 500;
    color: var(--wp-accent);
    flex: none;
  }
  .subject {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    color: var(--wp-text);
  }
  .meta {
    color: var(--wp-text-subtle);
    white-space: nowrap;
    flex: none;
  }
  .now {
    margin-left: auto;
    display: inline-flex;
    align-items: center;
    gap: 4px;
    flex: none;
    padding: 4px 8px;
    border-radius: var(--wp-r-sm);
    border: 1px solid var(--wp-border);
    background: var(--wp-card);
    color: var(--wp-text);
    font-size: 12px;
    cursor: pointer;
  }
  .now:hover {
    background: var(--wp-elevated);
  }
  .live {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    color: var(--wp-text-muted);
    font-family: var(--wp-font-mono);
  }
  .dot {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: var(--wp-accent);
  }
</style>
