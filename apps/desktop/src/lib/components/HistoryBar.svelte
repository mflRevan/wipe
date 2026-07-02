<script lang="ts">
  import { history, rewindCommit, enterRewind, returnToNow } from '$lib/stores/board';
  import Button from './ui/Button.svelte';
  import { formatDate } from '$lib/utils';
  import { History, ChevronLeft, ChevronRight } from 'lucide-svelte';

  // history[0] is the most recent commit. Slider index 0 == "now" (live HEAD),
  // index i (1..N) == history[i-1] (further back in time as i grows).
  let count = $derived($history.length);

  let index = $derived.by(() => {
    if (!$rewindCommit) return 0;
    const i = $history.findIndex((c) => c.hash === $rewindCommit!.hash);
    return i < 0 ? 0 : i + 1;
  });

  async function goto(i: number) {
    const clamped = Math.max(0, Math.min(count, i));
    if (clamped === 0) await returnToNow();
    else await enterRewind($history[clamped - 1]);
  }

  function onslider(e: Event) {
    void goto(Number((e.target as HTMLInputElement).value));
  }
</script>

<div class="flex items-center gap-3 rounded-xl border border-border bg-card px-4 py-2.5">
  <div class="flex items-center gap-2 text-sm font-medium text-muted-foreground">
    <History class="h-4 w-4 text-primary" />
    <span class="hidden sm:inline">Time machine</span>
  </div>

  <Button
    variant="ghost"
    size="icon"
    class="h-7 w-7"
    aria-label="Older commit"
    disabled={count === 0 || index >= count}
    onclick={() => goto(index + 1)}
  >
    <ChevronLeft class="h-4 w-4" />
  </Button>

  <input
    type="range"
    min="0"
    max={count}
    value={index}
    step="1"
    oninput={onslider}
    disabled={count === 0}
    aria-label="History position"
    class="h-1.5 flex-1 cursor-pointer appearance-none rounded-full bg-muted accent-primary"
  />

  <Button
    variant="ghost"
    size="icon"
    class="h-7 w-7"
    aria-label="Newer commit"
    disabled={index <= 0}
    onclick={() => goto(index - 1)}
  >
    <ChevronRight class="h-4 w-4" />
  </Button>

  <div class="w-40 shrink-0 text-right text-xs">
    {#if index === 0}
      <span class="inline-flex items-center gap-1.5 font-medium text-emerald-400">
        <span class="h-1.5 w-1.5 rounded-full bg-emerald-400"></span> Now (live)
      </span>
    {:else}
      {@const c = $history[index - 1]}
      <span class="block font-mono font-medium text-primary">{c?.short}</span>
      <span class="block truncate text-muted-foreground">{c ? formatDate(c.date) : ''}</span>
    {/if}
  </div>
</div>
