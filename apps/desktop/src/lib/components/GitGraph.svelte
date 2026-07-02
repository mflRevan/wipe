<script lang="ts">
  import { fade, scale } from 'svelte/transition';
  import { browser } from '$app/environment';
  import { X, RotateCcw, GitBranch, Tag } from 'lucide-svelte';
  import { graph, rewindCommit, enterRewind, returnToNow } from '$lib/stores/board';
  import { formatDate, LABEL_COLORS } from '$lib/utils';
  import type { GraphCommit } from '$lib/types';

  let { open = $bindable(false) }: { open?: boolean } = $props();

  const reduced = browser && matchMedia('(prefers-reduced-motion: reduce)').matches;
  const dur = reduced ? 0 : 200;

  const ROW = 40;
  const LANE = 22;
  const PAD = 18;
  const LANE_COLORS = Object.values(LABEL_COLORS);

  type Placed = GraphCommit & { row: number; lane: number };

  // Lane assignment (git log --graph style): newest first.
  let laid = $derived.by(() => {
    const commits = $graph;
    const pos = new Map<string, { row: number; lane: number }>();
    const active: (string | null)[] = [];
    const placed: Placed[] = [];

    const freeLane = () => {
      const i = active.indexOf(null);
      if (i !== -1) return i;
      active.push(null);
      return active.length - 1;
    };

    commits.forEach((c, row) => {
      let lane = active.indexOf(c.hash);
      if (lane === -1) {
        lane = freeLane();
        active[lane] = c.hash;
      }
      pos.set(c.hash, { row, lane });
      placed.push({ ...c, row, lane });

      // This lane (and any other lane reserved for the same hash) is now consumed.
      for (let i = 0; i < active.length; i++) if (active[i] === c.hash) active[i] = null;

      // First parent continues in the same lane if not already reserved elsewhere.
      const [p0, ...rest] = c.parents;
      if (p0 && !active.includes(p0)) active[lane] = p0;
      for (const pk of rest) {
        if (!active.includes(pk)) {
          const l = freeLane();
          active[l] = pk;
        }
      }
      // Trim trailing empty lanes.
      while (active.length && active[active.length - 1] === null) active.pop();
    });

    let maxLane = 0;
    for (const p of placed) maxLane = Math.max(maxLane, p.lane);

    // Edges from each commit to each of its (placed) parents. Node centers sit
    // at the vertical middle of each ROW-tall text row so lanes and text align.
    const edges: { x1: number; y1: number; x2: number; y2: number; lane: number }[] = [];
    const cx = (l: number) => PAD + l * LANE;
    const cy = (r: number) => ROW / 2 + r * ROW;
    for (const c of placed) {
      for (const p of c.parents) {
        const pp = pos.get(p);
        if (!pp) continue;
        edges.push({ x1: cx(c.lane), y1: cy(c.row), x2: cx(pp.lane), y2: cy(pp.row), lane: c.lane });
      }
    }

    return {
      placed,
      edges,
      width: PAD * 2 + maxLane * LANE,
      height: commits.length * ROW,
      cx,
      cy
    };
  });

  function laneColor(l: number): string {
    return LANE_COLORS[l % LANE_COLORS.length];
  }

  function edgePath(e: { x1: number; y1: number; x2: number; y2: number }): string {
    if (e.x1 === e.x2) return `M ${e.x1} ${e.y1} L ${e.x2} ${e.y2}`;
    const my = (e.y1 + e.y2) / 2;
    return `M ${e.x1} ${e.y1} C ${e.x1} ${my}, ${e.x2} ${my}, ${e.x2} ${e.y2}`;
  }

  function refKind(ref: string): 'tag' | 'branch' {
    return ref.startsWith('tag:') || ref.includes('tags/') ? 'tag' : 'branch';
  }
  function refLabel(ref: string): string {
    return ref.replace(/^tag:\s*/, '').replace(/^refs\/(heads|remotes|tags)\//, '');
  }

  async function pick(c: GraphCommit) {
    await enterRewind(c);
  }
</script>

{#if open}
  <div class="scrim" transition:fade={{ duration: dur }} onclick={() => (open = false)} role="presentation"></div>
  <div class="wrap">
    <div class="panel" transition:scale={{ duration: dur, start: 0.98, opacity: 0 }} role="dialog" aria-modal="true" aria-label="Commit history">
      <header class="head">
        <div class="htitle"><GitBranch size={16} /> <h3>History</h3></div>
        {#if $rewindCommit}
          <button class="now" onclick={returnToNow}><RotateCcw size={13} /> Return to now</button>
        {/if}
        <button class="close" aria-label="Close" onclick={() => (open = false)}><X size={18} /></button>
      </header>

      {#if $graph.length === 0}
        <div class="empty">No commit history available.</div>
      {:else}
        <div class="scroll wp-scroll">
          <div class="graph" style="height:{laid.height}px">
            <svg class="lanes" width={laid.width} height={laid.height} aria-hidden="true">
              {#each laid.edges as e (`${e.x1}-${e.y1}-${e.x2}-${e.y2}`)}
                <path d={edgePath(e)} stroke={laneColor(e.lane)} stroke-width="2" fill="none" opacity="0.55" />
              {/each}
              {#each laid.placed as c (c.hash)}
                {#if c.board}
                  <circle cx={laid.cx(c.lane)} cy={laid.cy(c.row)} r="6" fill={laneColor(c.lane)} stroke="var(--wp-card)" stroke-width="2" />
                {:else}
                  <circle cx={laid.cx(c.lane)} cy={laid.cy(c.row)} r="3.5" fill="var(--wp-card)" stroke={laneColor(c.lane)} stroke-width="1.5" />
                {/if}
              {/each}
            </svg>

            <div class="rows" style="left:{laid.width}px">
              {#each laid.placed as c (c.hash)}
                <button
                  class="crow"
                  class:checkpoint={c.board}
                  class:active={$rewindCommit?.hash === c.hash}
                  style="height:{ROW}px"
                  onclick={() => pick(c)}
                >
                  <span class="chash">{c.short}</span>
                  {#each c.refs as ref (ref)}
                    <span class="ref {refKind(ref)}">
                      {#if refKind(ref) === 'tag'}<Tag size={9} />{:else}<GitBranch size={9} />{/if}
                      {refLabel(ref)}
                    </span>
                  {/each}
                  <span class="csubj" class:muted={!c.board}>{c.subject}</span>
                  <span class="cmeta">{c.author_name} · {formatDate(c.date)}</span>
                </button>
              {/each}
            </div>
          </div>
        </div>
        <footer class="foot">
          <span class="legend"><span class="lg board"></span> board checkpoint</span>
          <span class="legend"><span class="lg other"></span> other commit</span>
          <span class="count">{$graph.filter((c) => c.board).length} checkpoints · {$graph.length} commits</span>
        </footer>
      {/if}
    </div>
  </div>
{/if}

<style>
  .scrim {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.5);
    z-index: 92;
  }
  .wrap {
    position: fixed;
    inset: 0;
    z-index: 93;
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 5vh 16px;
    pointer-events: none;
  }
  .panel {
    pointer-events: auto;
    display: flex;
    flex-direction: column;
    width: min(860px, 100%);
    max-height: 90vh;
    background: var(--wp-card);
    border: 1px solid var(--wp-border);
    border-radius: var(--wp-r-lg);
    box-shadow: var(--wp-shadow-lift);
    overflow: hidden;
  }
  .head {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 14px 16px;
    border-bottom: 1px solid var(--wp-border);
  }
  .htitle {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    color: var(--wp-text-muted);
  }
  .htitle h3 {
    font-family: var(--wp-font-display);
    font-size: 16px;
    font-weight: 600;
    color: var(--wp-text);
  }
  .now {
    margin-left: auto;
    display: inline-flex;
    align-items: center;
    gap: 4px;
    padding: 5px 10px;
    border-radius: var(--wp-r-sm);
    border: 1px solid var(--wp-accent);
    background: color-mix(in srgb, var(--wp-accent) 14%, transparent);
    color: var(--wp-accent);
    font-size: 12px;
    cursor: pointer;
  }
  .close {
    margin-left: auto;
    display: inline-flex;
    padding: 6px;
    border: none;
    background: none;
    color: var(--wp-text-muted);
    cursor: pointer;
    border-radius: var(--wp-r-sm);
  }
  .now + .close {
    margin-left: 0;
  }
  .close:hover {
    background: var(--wp-elevated);
    color: var(--wp-text);
  }
  .empty {
    padding: 40px;
    text-align: center;
    color: var(--wp-text-muted);
  }
  .scroll {
    flex: 1;
    overflow: auto;
    padding: 8px 0;
  }
  .graph {
    position: relative;
    min-width: 100%;
  }
  .lanes {
    position: absolute;
    top: 0;
    left: 0;
  }
  .rows {
    position: absolute;
    top: 0;
    right: 0;
  }
  .crow {
    display: flex;
    align-items: center;
    gap: 10px;
    width: 100%;
    padding: 0 16px 0 4px;
    border: none;
    background: none;
    color: var(--wp-text);
    cursor: pointer;
    text-align: left;
    border-radius: var(--wp-r-sm);
  }
  .crow:hover {
    background: var(--wp-elevated);
  }
  .crow.active {
    background: color-mix(in srgb, var(--wp-accent) 14%, transparent);
  }
  .chash {
    font-family: var(--wp-font-mono);
    font-size: 12px;
    color: var(--wp-accent);
    flex: none;
  }
  .crow:not(.checkpoint) .chash {
    color: var(--wp-text-subtle);
  }
  .ref {
    display: inline-flex;
    align-items: center;
    gap: 3px;
    flex: none;
    padding: 1px 6px;
    border-radius: var(--wp-r-pill);
    font-size: 10px;
    font-family: var(--wp-font-mono);
    border: 1px solid;
  }
  .ref.branch {
    color: var(--wp-focus);
    border-color: color-mix(in srgb, var(--wp-focus) 40%, transparent);
    background: color-mix(in srgb, var(--wp-focus) 12%, transparent);
  }
  .ref.tag {
    color: var(--wp-accent);
    border-color: color-mix(in srgb, var(--wp-accent) 40%, transparent);
    background: color-mix(in srgb, var(--wp-accent) 12%, transparent);
  }
  .csubj {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: 13px;
  }
  .csubj.muted {
    color: var(--wp-text-muted);
  }
  .cmeta {
    flex: none;
    font-family: var(--wp-font-mono);
    font-size: 11px;
    color: var(--wp-text-subtle);
  }
  .foot {
    display: flex;
    align-items: center;
    gap: 16px;
    padding: 10px 16px;
    border-top: 1px solid var(--wp-border);
    font-size: 12px;
    color: var(--wp-text-muted);
  }
  .legend {
    display: inline-flex;
    align-items: center;
    gap: 6px;
  }
  .lg {
    display: inline-block;
    border-radius: 50%;
  }
  .lg.board {
    width: 12px;
    height: 12px;
    background: var(--wp-accent);
  }
  .lg.other {
    width: 8px;
    height: 8px;
    border: 1.5px solid var(--wp-text-subtle);
  }
  .count {
    margin-left: auto;
    font-family: var(--wp-font-mono);
    font-size: 11px;
    color: var(--wp-text-subtle);
  }
</style>
