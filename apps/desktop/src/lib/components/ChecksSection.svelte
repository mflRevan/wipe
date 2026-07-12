<script lang="ts">
  import { Check, Plus, X, Icon as IconBase } from 'lucide-svelte';
  import { get } from 'svelte/store';
  import { api } from '$lib/api';
  import { currentProject, markSelfChange, applyTicket } from '$lib/stores/board';
  import type { ChecklistItem, ChecksKind, Ticket } from '$lib/types';

  let {
    kind,
    label,
    icon: Icon,
    ticketId,
    items,
    readOnly,
    placeholder,
    onerror
  }: {
    kind: ChecksKind;
    label: string;
    icon: typeof IconBase;
    ticketId: string;
    items: ChecklistItem[];
    readOnly: boolean;
    placeholder: string;
    onerror: (msg: string) => void;
  } = $props();

  let done = $derived(items.filter((i) => i.done).length);
  let total = $derived(items.length);

  let newItem = $state('');
  let editingItem = $state<string | null>(null);
  let editDraft = $state('');

  // Reset transient input state when the section is re-bound to another ticket.
  let boundTo = $state('');
  $effect(() => {
    if (ticketId !== boundTo) {
      boundTo = ticketId;
      newItem = '';
      editingItem = null;
      editDraft = '';
    }
  });

  function proj() {
    return get(currentProject) ?? undefined;
  }

  // All mutations route through here: suppress the board's self-flash and apply
  // the returned ticket right away so ticking/adding feels instant.
  async function mutate(fn: () => Promise<Ticket>) {
    markSelfChange(ticketId);
    try {
      applyTicket(await fn());
    } catch (e) {
      onerror(e instanceof Error ? e.message : String(e));
    }
  }
  async function addItem() {
    const t = newItem.trim();
    if (!t) return;
    newItem = '';
    await mutate(() => api.addCheckItem(kind, ticketId, t, proj()));
  }
  async function toggleItem(item: ChecklistItem) {
    await mutate(() => api.setCheckItem(kind, ticketId, item.id, { done: !item.done }, proj()));
  }
  async function removeItem(id: string) {
    await mutate(() => api.removeCheckItem(kind, ticketId, id, proj()));
  }
  function startEditItem(item: ChecklistItem) {
    if (readOnly) return;
    editingItem = item.id;
    editDraft = item.text;
  }
  async function saveEditItem(id: string) {
    const t = editDraft.trim();
    const cur = items.find((i) => i.id === id);
    editingItem = null;
    if (t && cur && t !== cur.text) {
      await mutate(() => api.setCheckItem(kind, ticketId, id, { text: t }, proj()));
    }
  }
</script>

<div class="field checks" class:acceptance={kind === 'acceptance'}>
  <div class="flabel-row">
    <span class="flabel"><Icon size={12} /> {label}</span>
    {#if total > 0}<span class="ck-count">{done}/{total}</span>{/if}
  </div>
  {#if total > 0}
    {#if kind === 'acceptance'}
      <!-- Criteria read as discrete gates, so the bar is segmented: one block
           per criterion, filled as the reviewer accepts each. -->
      <div class="ac-bar" aria-hidden="true">
        {#each items as item (item.id)}
          <div class="ac-seg" class:on={item.done}></div>
        {/each}
      </div>
    {:else}
      <div class="ck-bar" aria-hidden="true">
        <div class="ck-fill" style="width:{(done / total) * 100}%"></div>
      </div>
    {/if}
    <div class="ck-items">
      {#each items as item (item.id)}
        <div class="ck-item" class:done={item.done}>
          <button
            class="ck-check"
            role="checkbox"
            aria-checked={item.done}
            aria-label={item.done ? 'Uncheck item' : 'Check item'}
            disabled={readOnly}
            onclick={() => toggleItem(item)}
          >
            {#if item.done}<Check size={12} />{/if}
          </button>
          {#if editingItem === item.id && !readOnly}
            <!-- svelte-ignore a11y_autofocus -->
            <input
              class="ck-edit"
              autofocus
              bind:value={editDraft}
              onblur={() => saveEditItem(item.id)}
              onkeydown={(e) => {
                if (e.key === 'Enter') e.currentTarget.blur();
                else if (e.key === 'Escape') {
                  // Cancel just this item edit; don't let it bubble up and close
                  // the whole ticket modal.
                  e.preventDefault();
                  editingItem = null;
                }
              }}
            />
          {:else}
            <button class="ck-text" class:ro={readOnly} onclick={() => startEditItem(item)}
              >{item.text}</button
            >
          {/if}
          {#if !readOnly}
            <button class="ck-del" aria-label="Delete item" onclick={() => removeItem(item.id)}
              ><X size={13} /></button
            >
          {/if}
        </div>
      {/each}
    </div>
  {/if}
  {#if !readOnly}
    <div class="ck-add">
      <Plus size={14} class="ck-addicon" />
      <input
        class="ck-newinput"
        {placeholder}
        bind:value={newItem}
        onkeydown={(e) => {
          if (e.key === 'Enter') {
            e.preventDefault();
            void addItem();
          }
        }}
      />
      {#if newItem.trim()}
        <button class="ck-addbtn" onclick={addItem}>Add</button>
      {/if}
    </div>
  {/if}
</div>

<style>
  .field {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .flabel {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    width: fit-content;
    font-family: var(--wp-font-display);
    font-size: 11px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: var(--wp-text-muted);
  }
  .flabel-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }
  /* The acceptance surface gets its own hue (a calm sage, from the label
     palette) so "reviewer accepted" reads differently from "worker progress". */
  .checks {
    --ck-color: var(--wp-accent);
  }
  .checks.acceptance {
    --ck-color: #7e9b7a;
  }
  .ck-count {
    font-family: var(--wp-font-mono);
    font-size: 12px;
    color: var(--wp-text-subtle);
  }
  .ck-bar {
    height: 4px;
    border-radius: var(--wp-r-pill);
    background: var(--wp-surface);
    overflow: hidden;
  }
  .ck-fill {
    height: 100%;
    border-radius: var(--wp-r-pill);
    background: var(--ck-color);
    transition: width var(--wp-base) var(--wp-ease);
  }
  .ac-bar {
    display: flex;
    gap: 3px;
  }
  .ac-seg {
    flex: 1;
    height: 5px;
    border-radius: var(--wp-r-pill);
    background: var(--wp-surface);
    transition: background var(--wp-base) var(--wp-ease);
  }
  .ac-seg.on {
    background: var(--ck-color);
  }
  .ck-items {
    display: flex;
    flex-direction: column;
    gap: 1px;
  }
  .ck-item {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 3px 4px;
    border-radius: var(--wp-r-sm);
  }
  .ck-item:hover {
    background: var(--wp-surface);
  }
  .ck-check {
    flex: none;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 18px;
    height: 18px;
    border-radius: var(--wp-r-sm);
    border: 1.5px solid var(--wp-border-strong);
    background: var(--wp-card);
    color: var(--wp-on-accent);
    cursor: pointer;
    transition: all var(--wp-fast) var(--wp-ease);
  }
  /* Criteria checkboxes are round - a visual cue that they are review gates. */
  .acceptance .ck-check {
    border-radius: 50%;
  }
  .ck-check:hover:not(:disabled) {
    border-color: var(--ck-color);
  }
  .ck-check:disabled {
    cursor: default;
  }
  .ck-item.done .ck-check {
    background: var(--ck-color);
    border-color: var(--ck-color);
  }
  .ck-text {
    flex: 1;
    min-width: 0;
    text-align: left;
    padding: 2px 2px;
    border: none;
    background: none;
    color: var(--wp-text);
    font-size: 14px;
    line-height: 1.4;
    cursor: text;
    border-radius: var(--wp-r-sm);
    word-break: break-word;
  }
  .ck-text.ro {
    cursor: default;
  }
  .ck-item.done .ck-text {
    color: var(--wp-text-subtle);
    text-decoration: line-through;
  }
  /* Accepted criteria stay legible - dimmed, but never struck through. */
  .acceptance .ck-item.done .ck-text {
    text-decoration: none;
  }
  .ck-edit {
    flex: 1;
    min-width: 0;
    height: 26px;
    padding: 0 6px;
    border-radius: var(--wp-r-sm);
    border: 1px solid var(--wp-border-strong);
    background: var(--wp-card);
    color: var(--wp-text);
    font-size: 14px;
  }
  .ck-del {
    flex: none;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 22px;
    height: 22px;
    border: none;
    border-radius: var(--wp-r-sm);
    background: none;
    color: var(--wp-text-subtle);
    cursor: pointer;
    opacity: 0;
    transition: all var(--wp-fast) var(--wp-ease);
  }
  .ck-item:hover .ck-del {
    opacity: 1;
  }
  .ck-del:hover {
    background: var(--wp-elevated);
    color: var(--wp-error);
  }
  .ck-add {
    display: flex;
    align-items: center;
    gap: 6px;
    margin-top: 2px;
    padding: 0 4px;
  }
  :global(.ck-add .ck-addicon) {
    color: var(--wp-text-subtle);
    flex: none;
  }
  .ck-newinput {
    flex: 1;
    min-width: 0;
    height: 30px;
    padding: 0 6px;
    border: none;
    border-bottom: 1px solid transparent;
    background: none;
    color: var(--wp-text);
    font-size: 14px;
  }
  .ck-newinput:focus {
    outline: none;
    border-bottom-color: var(--wp-border-strong);
  }
  .ck-addbtn {
    flex: none;
    height: 26px;
    padding: 0 12px;
    border-radius: var(--wp-r-sm);
    border: none;
    background: var(--ck-color);
    color: var(--wp-on-accent);
    font-size: 13px;
    font-weight: 500;
    cursor: pointer;
  }
</style>
