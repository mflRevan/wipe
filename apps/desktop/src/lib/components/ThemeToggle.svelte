<script lang="ts">
  import { Sun, Moon, Monitor, Check } from 'lucide-svelte';
  import Popover from './ui/Popover.svelte';
  import {
    appearance,
    accent,
    setAppearance,
    setAccent,
    ACCENTS,
    type Appearance
  } from '$lib/stores/theme';

  const modes: { id: Appearance; label: string; icon: typeof Sun }[] = [
    { id: 'light', label: 'Light', icon: Sun },
    { id: 'dark', label: 'Dark', icon: Moon },
    { id: 'system', label: 'System', icon: Monitor }
  ];
</script>

<Popover align="end" width="220px">
  {#snippet trigger({ toggle })}
    <button class="tt-trigger" aria-label="Theme settings" title="Theme" onclick={toggle}>
      {#if $appearance === 'dark'}
        <Moon size={16} />
      {:else if $appearance === 'light'}
        <Sun size={16} />
      {:else}
        <Monitor size={16} />
      {/if}
    </button>
  {/snippet}
  {#snippet children()}
    <div class="section-label">Appearance</div>
    <div class="seg">
      {#each modes as m (m.id)}
        <button
          class="seg-btn"
          class:active={$appearance === m.id}
          onclick={() => setAppearance(m.id)}
        >
          <m.icon size={14} />
          <span>{m.label}</span>
        </button>
      {/each}
    </div>

    <div class="section-label">Accent</div>
    <div class="accents">
      {#each ACCENTS as a (a.id)}
        <button
          class="swatch"
          class:active={$accent === a.id}
          style="--sw:{a.hex}"
          title={a.name}
          aria-label={a.name}
          onclick={() => setAccent(a.id)}
        >
          {#if $accent === a.id}<Check size={13} />{/if}
        </button>
      {/each}
    </div>
  {/snippet}
</Popover>

<style>
  .tt-trigger {
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
  .tt-trigger:hover {
    color: var(--wp-text);
    background: var(--wp-elevated);
  }
  .section-label {
    font-family: var(--wp-font-display);
    font-size: 11px;
    font-weight: 600;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    color: var(--wp-text-muted);
    padding: 8px 6px 6px;
  }
  .seg {
    display: flex;
    gap: 4px;
    padding: 0 2px;
  }
  .seg-btn {
    flex: 1;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 3px;
    padding: 8px 4px;
    border-radius: var(--wp-r-sm);
    border: 1px solid var(--wp-border);
    background: var(--wp-card);
    color: var(--wp-text-muted);
    font-size: 11px;
    cursor: pointer;
    transition: all var(--wp-fast) var(--wp-ease);
  }
  .seg-btn:hover {
    background: var(--wp-elevated);
  }
  .seg-btn.active {
    border-color: var(--wp-accent);
    color: var(--wp-accent);
  }
  .accents {
    display: flex;
    gap: 8px;
    padding: 2px 6px 6px;
  }
  .swatch {
    width: 28px;
    height: 28px;
    border-radius: var(--wp-r-pill);
    background: var(--sw);
    border: 2px solid transparent;
    cursor: pointer;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    color: #fff;
    transition: transform var(--wp-fast) var(--wp-ease);
  }
  .swatch:hover {
    transform: scale(1.08);
  }
  .swatch.active {
    border-color: var(--wp-text);
  }
</style>
