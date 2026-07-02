<script lang="ts">
  import { fade, scale } from 'svelte/transition';
  import { browser } from '$app/environment';
  import { get } from 'svelte/store';
  import { X, Plus, Trash2 } from 'lucide-svelte';
  import Button from './ui/Button.svelte';
  import { api, getApiBase, setApiBase } from '$lib/api';
  import {
    board,
    health,
    definitions,
    currentProject,
    loadDefinitions,
    bootstrap,
    stopLiveUpdates
  } from '$lib/stores/board';
  import { labelColor, LABEL_COLORS, LABEL_KEYS } from '$lib/utils';

  let { open = $bindable(false) }: { open?: boolean } = $props();

  const reduced = browser && matchMedia('(prefers-reduced-motion: reduce)').matches;
  const dur = reduced ? 0 : 200;

  let apiBaseInput = $state('');
  let newLabel = $state('');
  let newColor = $state('terracotta');
  let recoloring = $state<string | null>(null);
  let error = $state<string | null>(null);

  function proj() {
    return get(currentProject) ?? undefined;
  }

  $effect(() => {
    if (open) apiBaseInput = getApiBase();
  });

  async function addLabel() {
    const name = newLabel.trim();
    if (!name) return;
    error = null;
    try {
      await api.createLabel(name, newColor, proj());
      await loadDefinitions();
      newLabel = '';
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    }
  }

  async function recolor(name: string, color: string) {
    error = null;
    try {
      await api.recolorLabel(name, color, proj());
      await loadDefinitions();
      recoloring = null;
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    }
  }

  async function del(name: string) {
    error = null;
    try {
      await api.deleteLabel(name, proj());
      await loadDefinitions();
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    }
  }

  async function saveApiBase() {
    setApiBase(apiBaseInput);
    open = false;
    stopLiveUpdates();
    await bootstrap();
  }

  function onKey(e: KeyboardEvent) {
    if (e.key === 'Escape' && open) open = false;
  }
</script>

<svelte:window onkeydown={onKey} />

{#if open}
  <div class="scrim" transition:fade={{ duration: dur }} onclick={() => (open = false)} role="presentation"></div>
  <div class="wrap">
    <div class="modal wp-scroll" transition:scale={{ duration: dur, start: 0.97, opacity: 0 }} role="dialog" aria-modal="true" aria-label="Board settings">
      <header class="head">
        <h3>Board settings</h3>
        <button class="close" aria-label="Close" onclick={() => (open = false)}><X size={18} /></button>
      </header>

      <div class="pad">
        <!-- board info -->
        <section class="sec">
          <span class="flabel">Board</span>
          <div class="info">
            <div class="irow"><span class="k">Name</span><span class="v">{$board?.board ?? '—'}</span></div>
            <div class="irow"><span class="k">Lists</span><span class="v">{$board?.lists.length ?? 0}</span></div>
            <div class="irow">
              <span class="k">Daemon</span>
              <span class="v">
                {#if $health}
                  <span class="dot ok"></span> {$health.service} v{$health.version}
                {:else}
                  <span class="dot off"></span> offline
                {/if}
              </span>
            </div>
          </div>
        </section>

        <!-- labels -->
        <section class="sec">
          <span class="flabel">Labels</span>
          <div class="labels">
            {#each $definitions.labels as label (label.name)}
              <div class="lrow">
                <button
                  class="sw"
                  aria-label="Change color for {label.name}"
                  title="Change color"
                  style="background:{labelColor(label.name, label.color)}"
                  onclick={() => (recoloring = recoloring === label.name ? null : label.name)}
                ></button>
                <span class="lname">{label.name}</span>
                <button class="del" aria-label="Delete {label.name}" title="Delete label" onclick={() => del(label.name)}>
                  <Trash2 size={14} />
                </button>
              </div>
              {#if recoloring === label.name}
                <div class="palette">
                  {#each LABEL_KEYS as key (key)}
                    <button
                      class="csw"
                      class:on={label.color === key}
                      style="background:{LABEL_COLORS[key]}"
                      title={key}
                      aria-label={key}
                      onclick={() => recolor(label.name, key)}
                    ></button>
                  {/each}
                </div>
              {/if}
            {/each}
            {#if $definitions.labels.length === 0}
              <div class="none">No labels yet.</div>
            {/if}
          </div>

          <div class="addlabel">
            <input
              class="in"
              placeholder="New label name"
              bind:value={newLabel}
              onkeydown={(e) => e.key === 'Enter' && addLabel()}
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
            <Button variant="primary" size="sm" onclick={addLabel}><Plus size={14} /> Add</Button>
          </div>
          <p class="hint">Rename isn't supported; delete strips the label from all tickets.</p>
        </section>

        <!-- connection -->
        <section class="sec">
          <span class="flabel">Connection</span>
          <input class="in" bind:value={apiBaseInput} placeholder="http://localhost:6737" />
          <p class="hint">Daemon API base URL. Stored locally; blank uses the serving origin.</p>
          <div class="actions">
            <Button variant="primary" size="sm" onclick={saveApiBase}>Save &amp; reconnect</Button>
          </div>
        </section>

        {#if error}<div class="err">{error}</div>{/if}
      </div>
    </div>
  </div>
{/if}

<style>
  .scrim {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.5);
    z-index: 90;
  }
  .wrap {
    position: fixed;
    inset: 0;
    z-index: 91;
    display: flex;
    align-items: flex-start;
    justify-content: center;
    padding: 6vh 16px;
    pointer-events: none;
    overflow-y: auto;
  }
  .modal {
    pointer-events: auto;
    width: min(560px, 100%);
    max-height: 88vh;
    overflow-y: auto;
    background: var(--wp-card);
    border: 1px solid var(--wp-border);
    border-radius: var(--wp-r-lg);
    box-shadow: var(--wp-shadow-lift);
  }
  .head {
    position: sticky;
    top: 0;
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 16px 20px;
    background: var(--wp-card);
    border-bottom: 1px solid var(--wp-border);
    z-index: 1;
  }
  .head h3 {
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
  .pad {
    padding: 18px 20px 22px;
    display: flex;
    flex-direction: column;
    gap: 22px;
  }
  .sec {
    display: flex;
    flex-direction: column;
    gap: 10px;
  }
  .flabel {
    font-family: var(--wp-font-display);
    font-size: 11px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: var(--wp-text-muted);
  }
  .info {
    display: flex;
    flex-direction: column;
    gap: 6px;
    border: 1px solid var(--wp-border);
    border-radius: var(--wp-r-md);
    padding: 12px;
    background: var(--wp-surface);
  }
  .irow {
    display: flex;
    justify-content: space-between;
    font-size: 13px;
  }
  .k {
    color: var(--wp-text-muted);
  }
  .v {
    display: inline-flex;
    align-items: center;
    gap: 6px;
  }
  .dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
  }
  .dot.ok {
    background: #7e9b7a;
  }
  .dot.off {
    background: var(--wp-error);
  }
  .labels {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .lrow {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 6px 4px;
  }
  .sw {
    width: 16px;
    height: 16px;
    border-radius: var(--wp-r-sm);
    border: 1px solid rgba(0, 0, 0, 0.15);
    padding: 0;
    flex: none;
    cursor: pointer;
  }
  .sw:hover {
    box-shadow: 0 0 0 2px var(--wp-border-strong);
  }
  .lname {
    flex: 1;
    font-size: 13px;
  }
  .del {
    display: inline-flex;
    padding: 5px;
    border: none;
    background: none;
    color: var(--wp-text-subtle);
    cursor: pointer;
    border-radius: var(--wp-r-sm);
  }
  .del:hover {
    color: var(--wp-error);
    background: color-mix(in srgb, var(--wp-error) 12%, transparent);
  }
  .palette,
  .swatches {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }
  .palette {
    padding: 0 4px 8px 30px;
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
  .none {
    font-size: 13px;
    color: var(--wp-text-subtle);
    padding: 4px;
  }
  .addlabel {
    display: flex;
    align-items: center;
    gap: 8px;
    flex-wrap: wrap;
    margin-top: 6px;
  }
  .in {
    height: 34px;
    padding: 0 10px;
    border-radius: var(--wp-r-sm);
    border: 1px solid var(--wp-border);
    background: var(--wp-canvas);
    color: var(--wp-text);
    flex: 1;
    min-width: 160px;
  }
  .hint {
    font-size: 12px;
    color: var(--wp-text-subtle);
    margin: 0;
  }
  .actions {
    display: flex;
    justify-content: flex-end;
  }
  .err {
    font-size: 12px;
    color: var(--wp-error);
  }
</style>
