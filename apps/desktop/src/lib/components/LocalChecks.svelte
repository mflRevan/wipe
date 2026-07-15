<script lang="ts">
  import { Check, Plus, X, Icon as IconBase } from 'lucide-svelte';
  import type { ChecklistItem } from '$lib/types';

  // A purely-local checklist/acceptance editor (no backend) for the create form.
  // Items get temporary ids; the create flow persists their text (and done state)
  // once the ticket exists.
  let {
    items = $bindable([] as ChecklistItem[]),
    label,
    icon: Icon,
    placeholder,
    accent = false
  }: {
    items?: ChecklistItem[];
    label: string;
    icon: typeof IconBase;
    placeholder: string;
    accent?: boolean;
  } = $props();

  let newItem = $state('');
  let seq = 0;
  let editing = $state<string | null>(null);
  let editDraft = $state('');

  let done = $derived(items.filter((i) => i.done).length);
  let total = $derived(items.length);

  function add() {
    const t = newItem.trim();
    if (!t) return;
    items = [...items, { id: `tmp-${seq++}`, text: t, done: false }];
    newItem = '';
  }
  function toggle(id: string) {
    items = items.map((i) => (i.id === id ? { ...i, done: !i.done } : i));
  }
  function remove(id: string) {
    items = items.filter((i) => i.id !== id);
  }
  function startEdit(item: ChecklistItem) {
    editing = item.id;
    editDraft = item.text;
  }
  function saveEdit(id: string) {
    const t = editDraft.trim();
    editing = null;
    if (t) items = items.map((i) => (i.id === id ? { ...i, text: t } : i));
  }
</script>

<div class="field checks" class:acceptance={accent}>
  <div class="flabel-row">
    <span class="flabel"><Icon size={12} /> {label}</span>
    {#if total > 0}<span class="ck-count">{done}/{total}</span>{/if}
  </div>
  {#if total > 0}
    {#if accent}
      <div class="ac-bar" aria-hidden="true">
        {#each items as item (item.id)}<div class="ac-seg" class:on={item.done}></div>{/each}
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
            aria-label={item.done ? 'Uncheck' : 'Check'}
            onclick={() => toggle(item.id)}
          >
            {#if item.done}<Check size={12} />{/if}
          </button>
          {#if editing === item.id}
            <!-- svelte-ignore a11y_autofocus -->
            <input
              class="ck-edit"
              autofocus
              bind:value={editDraft}
              onblur={() => saveEdit(item.id)}
              onkeydown={(e) => {
                if (e.key === 'Enter') e.currentTarget.blur();
                else if (e.key === 'Escape') {
                  e.preventDefault();
                  editing = null;
                }
              }}
            />
          {:else}
            <button class="ck-text" onclick={() => startEdit(item)}>{item.text}</button>
          {/if}
          <button class="ck-del" aria-label="Remove" onclick={() => remove(item.id)}>
            <X size={13} />
          </button>
        </div>
      {/each}
    </div>
  {/if}
  <div class="ck-add">
    <Plus size={14} class="ck-addicon" />
    <input
      class="ck-newinput"
      {placeholder}
      bind:value={newItem}
      onkeydown={(e) => {
        if (e.key === 'Enter') {
          e.preventDefault();
          add();
        }
      }}
    />
    {#if newItem.trim()}<button class="ck-addbtn" onclick={add}>Add</button>{/if}
  </div>
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
  .acceptance .ck-check {
    border-radius: 50%;
  }
  .ck-check:hover {
    border-color: var(--ck-color);
  }
  .ck-item.done .ck-check {
    background: var(--ck-color);
    border-color: var(--ck-color);
  }
  .ck-text {
    flex: 1;
    min-width: 0;
    text-align: left;
    padding: 2px;
    border: none;
    background: none;
    color: var(--wp-text);
    font-size: 14px;
    line-height: 1.4;
    cursor: text;
    border-radius: var(--wp-r-sm);
    word-break: break-word;
  }
  .ck-item.done .ck-text {
    color: var(--wp-text-subtle);
    text-decoration: line-through;
  }
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
