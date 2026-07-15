<script lang="ts">
  import { onMount } from 'svelte';
  import { get } from 'svelte/store';
  import { browser } from '$app/environment';
  import { MessageSquare, Paperclip, CheckSquare, ShieldCheck } from 'lucide-svelte';
  import Chip from './ui/Chip.svelte';
  import Avatar from './Avatar.svelte';
  import CardMenu from './CardMenu.svelte';
  import { definitions, identities, currentProject, recentlyChanged } from '$lib/stores/board';
  import { mediaUrl } from '$lib/api';
  import { labelColorFor, priorityColor, mediaKind } from '$lib/utils';
  import type { Attachment, Ticket } from '$lib/types';

  let { ticket, onopen }: { ticket: Ticket; onopen: (t: Ticket) => void } = $props();

  const reduced = browser && matchMedia('(prefers-reduced-motion: reduce)').matches;

  let dot = $derived(priorityColor(ticket.priority));
  // Checklist progress for the compact badge (only shown when there are items).
  let clTotal = $derived(ticket.checklist?.length ?? 0);
  let clDone = $derived(ticket.checklist?.filter((i) => i.done).length ?? 0);
  // Acceptance-criteria progress (reviewer gates), shown as its own badge.
  let acTotal = $derived(ticket.acceptance?.length ?? 0);
  let acDone = $derived(ticket.acceptance?.filter((i) => i.done).length ?? 0);
  // How this card just changed (drives the live-update animation).
  let change = $derived($recentlyChanged.get(ticket.id));

  // Title typewriter: when a card first appears because an agent/human just
  // created it, reveal the title character-by-character (as if being typed) for
  // ~1s. A freshly-created ticket is a freshly-mounted node, so we read the change
  // kind at mount and animate once; otherwise the title renders normally.
  let typing = $state(false);
  let typed = $state('');
  let caret = $state(false);
  onMount(() => {
    if (reduced || get(recentlyChanged).get(ticket.id) !== 'new') return;
    const full = ticket.title;
    typing = true;
    caret = true;
    typed = '';
    let i = 0;
    const step = Math.min(45, Math.max(14, 900 / Math.max(full.length, 1)));
    const iv = setInterval(() => {
      i += 1;
      typed = full.slice(0, i);
      if (i >= full.length) {
        clearInterval(iv);
        typing = false;
        setTimeout(() => (caret = false), 350);
      }
    }, step);
    return () => clearInterval(iv);
  });
  // First image attachment becomes a compact card cover, like Trello.
  let cover = $derived<Attachment | undefined>(
    ticket.attachments.find((a) => mediaKind(a.mime, a.name) === 'image')
  );
  // Resolve an assignee to its identity. Identities are keyed by bare email, but
  // assignees are often stored as the full git form `Name <email>` — so fall back
  // to matching the address inside the angle brackets (else the avatar/name/kind
  // never resolve for git-human assignees).
  function identityFor(id: string) {
    const direct = $identities.find((i) => i.id === id);
    if (direct) return direct;
    const m = id.match(/<([^>]+)>/);
    return m ? $identities.find((i) => i.id === m[1]) : undefined;
  }
</script>

<div
  class="card"
  class:edited={change === 'edited'}
  class:materialize={change === 'new'}
  class:floated={change === 'moved'}
  role="button"
  tabindex="0"
  onclick={() => onopen(ticket)}
  onkeydown={(e) => (e.key === 'Enter' ? onopen(ticket) : null)}
>
  <div class="card-menu"><CardMenu {ticket} /></div>

  {#if cover}
    <div class="cover">
      <img
        src={mediaUrl(cover.path, $currentProject ?? undefined)}
        alt={cover.name}
        loading="lazy"
      />
    </div>
  {/if}

  {#if ticket.labels.length}
    <div class="chips">
      {#each ticket.labels as label (label)}
        <Chip color={labelColorFor(label, $definitions.labels)}>{label}</Chip>
      {/each}
    </div>
  {/if}

  <div class="title">
    {#if ticket.priority}
      <span class="prio" style="--d:{dot}" title="Priority: {ticket.priority}"></span>
    {/if}
    <span class="ttext"
      >{typing ? typed : ticket.title}{#if caret}<span class="caret" aria-hidden="true"></span
        >{/if}</span
    >
  </div>

  <div class="footer">
    <div class="avatars">
      {#each ticket.assignees.slice(0, 4) as a (a)}
        <Avatar id={a} identity={identityFor(a)} size={22} />
      {/each}
      {#if ticket.assignees.length > 4}
        <span class="more">+{ticket.assignees.length - 4}</span>
      {/if}
    </div>
    <div class="counts">
      {#if clTotal}
        <span
          class="count"
          class:cl-complete={clDone === clTotal}
          title="Checklist: {clDone} of {clTotal} done"
        >
          <CheckSquare size={13} /> {clDone}/{clTotal}
        </span>
      {/if}
      {#if acTotal}
        <span
          class="count ac"
          class:ac-complete={acDone === acTotal}
          title="Acceptance criteria: {acDone} of {acTotal} met"
        >
          <ShieldCheck size={13} /> {acDone}/{acTotal}
        </span>
      {/if}
      {#if ticket.comments.length}
        <span class="count"><MessageSquare size={13} /> {ticket.comments.length}</span>
      {/if}
      {#if ticket.attachments.length}
        <span class="count"><Paperclip size={13} /> {ticket.attachments.length}</span>
      {/if}
    </div>
  </div>
</div>

<style>
  .card {
    position: relative;
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 12px;
    background: var(--wp-card);
    border: 1px solid var(--wp-border);
    border-radius: var(--wp-r-md);
    cursor: pointer;
    transition:
      background var(--wp-fast) var(--wp-ease),
      border-color var(--wp-fast) var(--wp-ease),
      box-shadow var(--wp-fast) var(--wp-ease);
  }
  /* Quick-actions meatballs, top-right; revealed on card hover (or when open). */
  .card-menu {
    position: absolute;
    top: 8px;
    right: 8px;
    z-index: 3;
  }
  .card:hover :global(.card-menu .cm-btn),
  .card:focus-within :global(.card-menu .cm-btn) {
    opacity: 1;
  }
  .card:hover {
    background: var(--wp-elevated);
    box-shadow: var(--wp-shadow);
  }
  /* Live updates: three motions by how the card changed.
     - edited: a brief accent highlight (an agent/human tweaked it).
     - new: it materializes into place, as if just written (pairs with the title
       typewriter above).
     - moved: it floats in with a highlight when it lands in a new list. */
  .card.edited {
    animation: card-flash 1.8s var(--wp-ease);
  }
  .card.materialize {
    animation: card-materialize 0.5s var(--wp-ease) both;
  }
  .card.floated {
    animation: card-float 0.6s var(--wp-ease);
  }
  @keyframes card-flash {
    0% {
      border-color: var(--wp-accent);
      box-shadow: 0 0 0 3px color-mix(in srgb, var(--wp-accent) 30%, transparent);
    }
    100% {
      border-color: var(--wp-border);
      box-shadow: none;
    }
  }
  @keyframes card-materialize {
    0% {
      opacity: 0;
      transform: translateY(6px) scale(0.96);
      border-color: var(--wp-accent);
      box-shadow: 0 0 0 3px color-mix(in srgb, var(--wp-accent) 26%, transparent);
    }
    60% {
      opacity: 1;
      transform: translateY(0) scale(1);
    }
    100% {
      transform: none;
      border-color: var(--wp-border);
      box-shadow: none;
    }
  }
  @keyframes card-float {
    0% {
      transform: translateY(-6px);
      border-color: var(--wp-accent);
      box-shadow:
        0 8px 18px -8px color-mix(in srgb, var(--wp-accent) 45%, transparent),
        0 0 0 2px color-mix(in srgb, var(--wp-accent) 35%, transparent);
    }
    100% {
      transform: none;
      border-color: var(--wp-border);
      box-shadow: none;
    }
  }
  /* The typewriter caret while a new card's title is being "typed". */
  .caret {
    display: inline-block;
    width: 2px;
    height: 1em;
    margin-left: 1px;
    transform: translateY(2px);
    background: var(--wp-accent);
    animation: caret-blink 0.7s steps(1) infinite;
  }
  @keyframes caret-blink {
    50% {
      opacity: 0;
    }
  }
  @media (prefers-reduced-motion: reduce) {
    .card.edited,
    .card.materialize,
    .card.floated {
      animation: none;
      border-color: var(--wp-accent);
    }
    .caret {
      display: none;
    }
  }
  .cover {
    margin: -12px -12px 0;
    max-height: 120px;
    overflow: hidden;
    border-radius: var(--wp-r-md) var(--wp-r-md) 0 0;
    background: var(--wp-surface);
  }
  .cover img {
    display: block;
    width: 100%;
    max-height: 120px;
    object-fit: cover;
  }
  .prio {
    width: 8px;
    height: 8px;
    margin-top: 5px;
    border-radius: 50%;
    background: var(--d);
    flex: none;
  }
  .title {
    display: flex;
    align-items: flex-start;
    gap: 6px;
    font-size: 14px;
    font-weight: 500;
    letter-spacing: -0.005em;
    line-height: 1.4;
    color: var(--wp-text);
  }
  .ttext {
    display: -webkit-box;
    -webkit-line-clamp: 3;
    line-clamp: 3;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }
  .chips {
    display: flex;
    flex-wrap: wrap;
    gap: 4px;
  }
  .footer {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    min-height: 22px;
  }
  .avatars {
    display: flex;
    align-items: center;
  }
  .avatars > :global(*:not(:first-child)) {
    margin-left: -6px;
  }
  .more {
    margin-left: 4px;
    font-size: 11px;
    color: var(--wp-text-subtle);
  }
  .counts {
    display: flex;
    gap: 10px;
    color: var(--wp-text-subtle);
  }
  .count {
    display: inline-flex;
    align-items: center;
    gap: 3px;
    font-size: 12px;
    font-family: var(--wp-font-mono);
  }
  /* All items checked - nudge the badge toward the accent so it reads as "done". */
  .count.cl-complete {
    color: var(--wp-accent);
  }
  /* All acceptance criteria met - reads in the criteria's own (sage) hue. */
  .count.ac-complete {
    color: #7e9b7a;
  }
</style>
