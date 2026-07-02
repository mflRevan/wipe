<script lang="ts">
  import { Tag, Plus, Check } from 'lucide-svelte';
  import { get } from 'svelte/store';
  import Popover from './ui/Popover.svelte';
  import Chip from './ui/Chip.svelte';
  import { api } from '$lib/api';
  import { definitions, currentProject, loadDefinitions } from '$lib/stores/board';
  import { labelColor, LABEL_COLORS, LABEL_KEYS } from '$lib/utils';

  let { selected, onchange }: { selected: string[]; onchange: (labels: string[]) => void } =
    $props();

  let creating = $state(false);
  let newName = $state('');
  let newColor = $state<string>('terracotta');
  let error = $state<string | null>(null);

  function toggle(name: string) {
    const next = selected.includes(name)
      ? selected.filter((l) => l !== name)
      : [...selected, name];
    onchange(next);
  }

  async function create() {
    const name = newName.trim();
    if (!name) return;
    error = null;
    try {
      await api.createLabel(name, newColor, undefined, get(currentProject) ?? undefined);
      await loadDefinitions();
      if (!selected.includes(name)) onchange([...selected, name]);
      newName = '';
      creating = false;
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    }
  }
</script>

<div class="lp">
  <div class="chips">
    {#each selected as name (name)}
      <Chip
        color={labelColor(name, $definitions.labels.find((l) => l.name === name)?.color)}
        onremove={() => toggle(name)}>{name}</Chip
      >
    {/each}
    <Popover width="240px">
      {#snippet trigger({ toggle: t })}
        <button class="edit" onclick={t} title="Edit labels">
          <Tag size={12} /> Labels
        </button>
      {/snippet}
      {#snippet children()}
        <div class="head">Labels</div>
        {#each $definitions.labels as label (label.name)}
          <button class="row" onclick={() => toggle(label.name)}>
            <span class="sw" style="background:{labelColor(label.name, label.color)}"></span>
            <span class="rname">{label.name}</span>
            {#if selected.includes(label.name)}<Check size={14} />{/if}
          </button>
        {/each}
        {#if $definitions.labels.length === 0}
          <div class="none">No labels yet</div>
        {/if}

        <div class="divider"></div>
        {#if creating}
          <div class="create">
            <input
              class="ci"
              placeholder="Label name"
              bind:value={newName}
              onkeydown={(e) => e.key === 'Enter' && create()}
            />
            <div class="swatches">
              {#each LABEL_KEYS as key (key)}
                <button
                  class="csw"
                  class:on={newColor === key}
                  style="background:{LABEL_COLORS[key]}"
                  title={key}
                  aria-label={key}
                  onclick={() => (newColor = key)}
                ></button>
              {/each}
            </div>
            {#if error}<div class="err">{error}</div>{/if}
            <div class="cactions">
              <button class="ghost" onclick={() => (creating = false)}>Cancel</button>
              <button class="prim" onclick={create}>Create</button>
            </div>
          </div>
        {:else}
          <button class="row add" onclick={() => (creating = true)}>
            <Plus size={14} /> New label
          </button>
        {/if}
      {/snippet}
    </Popover>
  </div>
</div>

<style>
  .chips {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    align-items: center;
  }
  .edit {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    height: 22px;
    padding: 0 8px;
    border-radius: var(--wp-r-sm);
    border: 1px dashed var(--wp-border-strong);
    background: none;
    color: var(--wp-text-muted);
    font-size: 11px;
    cursor: pointer;
  }
  .edit:hover {
    color: var(--wp-text);
    background: var(--wp-elevated);
  }
  .head {
    font-size: 11px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: var(--wp-text-muted);
    padding: 4px 6px 6px;
  }
  .row {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
    padding: 7px 8px;
    border: none;
    background: none;
    color: var(--wp-text);
    border-radius: var(--wp-r-sm);
    cursor: pointer;
    font-size: 13px;
  }
  .row:hover {
    background: var(--wp-elevated);
  }
  .rname {
    flex: 1;
    text-align: left;
  }
  .sw {
    width: 12px;
    height: 12px;
    border-radius: var(--wp-r-sm);
    flex: none;
  }
  .add {
    color: var(--wp-text-muted);
  }
  .divider {
    height: 1px;
    background: var(--wp-border);
    margin: 6px 0;
  }
  .none {
    padding: 6px 8px;
    font-size: 12px;
    color: var(--wp-text-subtle);
  }
  .create {
    padding: 4px 6px;
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .ci {
    height: 30px;
    padding: 0 8px;
    border-radius: var(--wp-r-sm);
    border: 1px solid var(--wp-border);
    background: var(--wp-card);
  }
  .swatches {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }
  .csw {
    width: 20px;
    height: 20px;
    border-radius: var(--wp-r-sm);
    border: 2px solid transparent;
    cursor: pointer;
  }
  .csw.on {
    border-color: var(--wp-text);
  }
  .err {
    font-size: 11px;
    color: var(--wp-error);
  }
  .cactions {
    display: flex;
    justify-content: flex-end;
    gap: 6px;
  }
  .ghost,
  .prim {
    height: 28px;
    padding: 0 10px;
    border-radius: var(--wp-r-sm);
    font-size: 12px;
    cursor: pointer;
    border: 1px solid var(--wp-border);
  }
  .ghost {
    background: none;
    color: var(--wp-text-muted);
  }
  .prim {
    background: var(--wp-accent);
    color: var(--wp-on-accent);
    border-color: transparent;
  }
</style>
