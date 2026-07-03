<script lang="ts">
  import { Users, Check, Plus, Pencil, Bot, User, Trash2 } from 'lucide-svelte';
  import { get } from 'svelte/store';
  import Popover from './ui/Popover.svelte';
  import Avatar from './Avatar.svelte';
  import { api } from '$lib/api';
  import { identities, currentProject, loadIdentities } from '$lib/stores/board';
  import type { Identity } from '$lib/types';

  let { selected, onchange }: { selected: string[]; onchange: (a: string[]) => void } = $props();

  let editingId = $state<string | null>(null);
  let editName = $state('');
  let addingAgent = $state(false);
  let agentId = $state('');
  let agentName = $state('');
  let error = $state<string | null>(null);

  function identityFor(id: string): Identity | undefined {
    return $identities.find((i) => i.id === id);
  }

  function toggle(id: string) {
    const next = selected.includes(id) ? selected.filter((a) => a !== id) : [...selected, id];
    onchange(next);
  }

  function startEdit(i: Identity) {
    editingId = i.id;
    editName = i.display_name;
  }

  async function saveEdit(i: Identity) {
    const name = editName.trim();
    if (name && name !== i.display_name) {
      try {
        await api.putIdentity(i.id, name, i.kind, get(currentProject) ?? undefined);
        await loadIdentities();
      } catch (e) {
        error = e instanceof Error ? e.message : String(e);
      }
    }
    editingId = null;
  }

  async function removeIdentity(i: Identity) {
    error = null;
    try {
      await api.deleteIdentity(i.id, get(currentProject) ?? undefined);
      if (selected.includes(i.id)) onchange(selected.filter((a) => a !== i.id));
      await loadIdentities();
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    }
  }

  async function addAgent() {
    const id = agentId.trim();
    const name = agentName.trim() || id;
    if (!id) return;
    error = null;
    try {
      await api.putIdentity(id, name, 'agent', get(currentProject) ?? undefined);
      await loadIdentities();
      if (!selected.includes(id)) onchange([...selected, id]);
      agentId = '';
      agentName = '';
      addingAgent = false;
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    }
  }
</script>

<div class="ap">
  <div class="row-selected">
    {#each selected as id (id)}
      <span class="picked">
        <Avatar {id} identity={identityFor(id)} size={22} />
        <span class="pname">{identityFor(id)?.display_name ?? id}</span>
        <button class="rm" aria-label="Unassign" onclick={() => toggle(id)}>×</button>
      </span>
    {/each}

    <Popover width="280px">
      {#snippet trigger({ toggle: t })}
        <button class="edit" onclick={t} title="Assignees">
          <Users size={12} /> Assign
        </button>
      {/snippet}
      {#snippet children()}
        <div class="head">Identities</div>
        {#each $identities as i (i.id)}
          <div class="irow">
            <button class="pick" onclick={() => toggle(i.id)}>
              <Avatar id={i.id} identity={i} size={24} />
              {#if editingId === i.id}
                <input
                  class="ei"
                  bind:value={editName}
                  onclick={(e) => e.stopPropagation()}
                  onkeydown={(e) => e.key === 'Enter' && saveEdit(i)}
                  onblur={() => saveEdit(i)}
                />
              {:else}
                <span class="col">
                  <span class="dn">{i.display_name}</span>
                  <span class="kind">
                    {#if i.kind === 'agent'}<Bot size={10} /> agent{:else}<User size={10} /> human{/if}
                    · {i.id}
                  </span>
                </span>
              {/if}
              {#if selected.includes(i.id)}<Check size={15} />{/if}
            </button>
            {#if editingId !== i.id}
              <button
                class="pencil"
                aria-label="Rename"
                onclick={(e) => {
                  e.stopPropagation();
                  startEdit(i);
                }}
              >
                <Pencil size={13} />
              </button>
              {#if i.kind === 'agent'}
                <button
                  class="pencil danger"
                  aria-label="Remove identity"
                  title="Remove agent identity"
                  onclick={(e) => {
                    e.stopPropagation();
                    removeIdentity(i);
                  }}
                >
                  <Trash2 size={13} />
                </button>
              {/if}
            {/if}
          </div>
        {/each}
        {#if $identities.length === 0}
          <div class="none">No identities discovered yet</div>
        {/if}

        <div class="divider"></div>
        {#if addingAgent}
          <div class="create">
            <input class="ci" placeholder="agent id (e.g. claude)" bind:value={agentId} />
            <input class="ci" placeholder="display name (optional)" bind:value={agentName} />
            {#if error}<div class="err">{error}</div>{/if}
            <div class="cactions">
              <button class="ghost" onclick={() => (addingAgent = false)}>Cancel</button>
              <button class="prim" onclick={addAgent}>Add agent</button>
            </div>
          </div>
        {:else}
          <button class="addrow" onclick={() => (addingAgent = true)}>
            <Plus size={14} /> Add agent identity
          </button>
        {/if}
      {/snippet}
    </Popover>
  </div>
</div>

<style>
  .row-selected {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    align-items: center;
  }
  .picked {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    height: 26px;
    padding: 0 6px 0 3px;
    border-radius: var(--wp-r-pill);
    background: var(--wp-surface);
    border: 1px solid var(--wp-border);
  }
  .pname {
    font-size: 12px;
    color: var(--wp-text);
  }
  .rm {
    background: none;
    border: none;
    color: var(--wp-text-subtle);
    cursor: pointer;
    font-size: 15px;
    line-height: 1;
    padding: 0;
  }
  .rm:hover {
    color: var(--wp-text);
  }
  .edit {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    height: 26px;
    padding: 0 10px;
    border-radius: var(--wp-r-pill);
    border: 1px dashed var(--wp-border-strong);
    background: none;
    color: var(--wp-text-muted);
    font-size: 12px;
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
  .irow {
    display: flex;
    align-items: center;
  }
  .pick {
    display: flex;
    align-items: center;
    gap: 8px;
    flex: 1;
    padding: 6px 8px;
    border: none;
    background: none;
    color: var(--wp-text);
    border-radius: var(--wp-r-sm);
    cursor: pointer;
    text-align: left;
    min-width: 0;
  }
  .pick:hover {
    background: var(--wp-elevated);
  }
  .col {
    display: flex;
    flex-direction: column;
    gap: 1px;
    flex: 1;
    min-width: 0;
  }
  .dn {
    font-size: 13px;
  }
  .kind {
    display: inline-flex;
    align-items: center;
    gap: 3px;
    font-family: var(--wp-font-mono);
    font-size: 10px;
    color: var(--wp-text-subtle);
  }
  .ei,
  .ci {
    height: 30px;
    padding: 0 8px;
    border-radius: var(--wp-r-sm);
    border: 1px solid var(--wp-border-strong);
    background: var(--wp-card);
    flex: 1;
    min-width: 0;
  }
  .pencil {
    padding: 6px;
    border: none;
    background: none;
    color: var(--wp-text-subtle);
    cursor: pointer;
  }
  .pencil:hover {
    color: var(--wp-text);
  }
  .pencil.danger:hover {
    color: var(--wp-error);
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
  .addrow {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
    padding: 7px 8px;
    border: none;
    background: none;
    color: var(--wp-text-muted);
    border-radius: var(--wp-r-sm);
    cursor: pointer;
    font-size: 13px;
  }
  .addrow:hover {
    background: var(--wp-elevated);
  }
  .create {
    padding: 4px 6px;
    display: flex;
    flex-direction: column;
    gap: 8px;
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
