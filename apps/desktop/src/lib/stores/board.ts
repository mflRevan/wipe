import { writable, derived, get } from 'svelte/store';
import { api, subscribeChanges } from '$lib/api';
import { applyServerDefaults } from '$lib/stores/theme';
import type { Board, Definitions, GraphCommit, Health, Identity, Project, Ticket } from '$lib/types';

export const health = writable<Health | null>(null);
export const healthError = writable<string | null>(null);

export const projects = writable<Project[]>([]);
export const currentProject = writable<string | null>(null);

export const board = writable<Board | null>(null);
export const boardError = writable<string | null>(null);
export const loading = writable<boolean>(false);

// --- persistence: keep the open board across a page refresh --------------------
const PROJECT_KEY = 'wipe:project';
function lsGet(key: string): string | null {
  try {
    return typeof localStorage !== 'undefined' ? localStorage.getItem(key) : null;
  } catch {
    return null;
  }
}
function lsSet(key: string, value: string | null): void {
  try {
    if (typeof localStorage === 'undefined') return;
    if (value) localStorage.setItem(key, value);
    else localStorage.removeItem(key);
  } catch {
    /* storage unavailable / disabled - persistence is best-effort */
  }
}
// Capture the last-open board BEFORE wiring the subscriber below: subscribe fires
// synchronously with the initial `null`, which would otherwise clear the value we
// need to restore.
const savedProject = lsGet(PROJECT_KEY);
currentProject.subscribe((v) => lsSet(PROJECT_KEY, v));

export const definitions = writable<Definitions>({
  version: 0,
  labels: [],
  priorities: []
});
export const identities = writable<Identity[]>([]);

/** Ticket ids that changed since the last poll (drives the "just changed" flash).
 *  Ids auto-expire, so this is a rolling set of very recent changes. */
export const recentlyChanged = writable<Set<string>>(new Set());
/** True when the forum has new posts the viewer hasn't looked at yet. */
export const forumUnread = writable<boolean>(false);

const changeTimers = new Map<string, ReturnType<typeof setTimeout>>();
// Ticket ids the local user just acted on; their next diff is NOT flashed (the
// flash is meant for remote/agent changes, not your own drag/edit).
const selfActed = new Set<string>();
const selfTimers = new Map<string, ReturnType<typeof setTimeout>>();

/** Suppress the change-flash for a ticket the local user just modified. */
export function markSelfChange(id: string): void {
  selfActed.add(id);
  const prev = selfTimers.get(id);
  if (prev) clearTimeout(prev);
  selfTimers.set(
    id,
    setTimeout(() => {
      selfActed.delete(id);
      selfTimers.delete(id);
    }, 2500)
  );
}

/** Clear all pending flash timers and the highlight set (on project switch / rewind). */
function clearChanges(): void {
  for (const t of changeTimers.values()) clearTimeout(t);
  changeTimers.clear();
  recentlyChanged.set(new Set());
}

/** Flag a ticket as just-changed for ~1.8s so the UI can highlight it. */
function flashTicket(id: string): void {
  recentlyChanged.update((s) => {
    const n = new Set(s);
    n.add(id);
    return n;
  });
  const prev = changeTimers.get(id);
  if (prev) clearTimeout(prev);
  changeTimers.set(
    id,
    setTimeout(() => {
      recentlyChanged.update((s) => {
        const n = new Set(s);
        n.delete(id);
        return n;
      });
      changeTimers.delete(id);
    }, 1800)
  );
}

/** Diff two board snapshots and flash tickets that appeared, moved, or were edited. */
function markChanges(prev: Board, next: Board): void {
  const before = new Map<string, { updated?: string; list: string }>();
  for (const l of prev.lists) for (const t of l.tickets) before.set(t.id, { updated: t.updated, list: l.list });
  for (const l of next.lists) {
    for (const t of l.tickets) {
      const p = before.get(t.id);
      if ((!p || p.updated !== t.updated || p.list !== l.list) && !selfActed.has(t.id)) {
        flashTicket(t.id);
      }
    }
  }
}

// Rewind / history state - driven by the repository-wide commit graph.
export const graph = writable<GraphCommit[]>([]);
export const rewindCommit = writable<GraphCommit | null>(null);
export const rewinding = derived(rewindCommit, ($c) => $c !== null);

/** Flat list of every ticket on the current board (for drawer lookup). */
export const allTickets = derived(board, ($b) =>
  $b ? $b.lists.flatMap((l) => l.tickets) : ([] as Ticket[])
);

let unsubscribeWs: (() => void) | null = null;

function project(): string | undefined {
  return get(currentProject) ?? undefined;
}

/** Poll health; safe to call repeatedly. */
export async function checkHealth(): Promise<boolean> {
  try {
    const h = await api.health();
    health.set(h);
    healthError.set(null);
    return true;
  } catch (e) {
    health.set(null);
    healthError.set(e instanceof Error ? e.message : String(e));
    return false;
  }
}

/** Load the project list and pick a current project if none is set. */
export async function loadProjects(): Promise<void> {
  try {
    const { projects: list, current: served } = await api.projects();
    projects.set(list);
    const cur = get(currentProject);
    if (!cur || !list.some((p) => p.path === cur)) {
      // Prefer the board we were last viewing (survives a page refresh), then the
      // board `wipe serve` was launched in, then the first registered board.
      const has = (p: string | null | undefined): p is string =>
        !!p && list.some((x) => x.path === p);
      const pick = has(savedProject)
        ? savedProject
        : has(served)
          ? served
          : (list[0]?.path ?? null);
      if (pick) currentProject.set(pick);
    }
  } catch (e) {
    boardError.set(e instanceof Error ? e.message : String(e));
  }
}

// Request ordering + provenance for the board. Because the 0.5s poll and the WS
// handler both fetch concurrently, responses can arrive out of order or after the
// user has switched projects / entered history; these guard against applying a
// stale or misattributed snapshot.
let boardIssued = 0;
let boardApplied = 0;
/** The project the currently-stored `board` snapshot belongs to (null when it's a
 *  historical snapshot or nothing is loaded), so we only diff-for-flash within one
 *  project's live timeline. */
let boardProject: string | null = null;

/** Fetch the live board for the current project.
 *
 * `silent` (used by the 0.5s poll) skips the loading spinner and, on error, keeps
 * the current board rather than surfacing a transient blip. The store is only
 * updated when the board actually changed, so idle polls cause no re-render; when
 * it did change, the diff drives the just-changed highlight. */
export async function loadBoard(opts: { silent?: boolean } = {}): Promise<void> {
  const silent = opts.silent ?? false;
  const proj = project();
  const seq = ++boardIssued;
  if (!silent) loading.set(true);
  try {
    const next = await api.board(proj);
    // Drop responses that lost the race (a newer one already applied), that belong
    // to a project we've since left, or that would clobber a history snapshot.
    if (seq <= boardApplied) return;
    if (project() !== proj) return;
    if (get(rewindCommit)) return;
    boardApplied = seq;
    const prev = get(board);
    if (!prev || JSON.stringify(prev) !== JSON.stringify(next)) {
      // Only flash diffs against a live snapshot of the SAME project - never when
      // switching boards or returning from history (which would light up every card).
      if (prev && boardProject === proj) markChanges(prev, next);
      board.set(next);
    }
    boardProject = proj ?? null;
    boardError.set(null);
  } catch (e) {
    if (!silent) boardError.set(e instanceof Error ? e.message : String(e));
  } finally {
    if (!silent) loading.set(false);
  }
}

export async function loadDefinitions(): Promise<void> {
  try {
    definitions.set(await api.definitions(project()));
  } catch {
    /* definitions are non-critical */
  }
}

export async function loadIdentities(): Promise<void> {
  try {
    identities.set(await api.identities(project()));
  } catch {
    /* identities are non-critical */
  }
}

/** Fetch the repository-wide commit graph for the current project. */
export async function loadGraph(): Promise<void> {
  try {
    graph.set(await api.graph(project()));
  } catch {
    /* graph is non-critical; degrade quietly when unavailable */
  }
}

/** Enter rewind mode at a specific commit and load the historical snapshot. */
export async function enterRewind(commit: GraphCommit): Promise<void> {
  loading.set(true);
  try {
    // Set rewind first so any in-flight live poll bails before clobbering the
    // snapshot, and mark the stored board as historical (no flash diffing).
    rewindCommit.set(commit);
    board.set(await api.boardAt(commit.hash, project()));
    boardProject = null;
    clearChanges();
    boardError.set(null);
  } catch (e) {
    boardError.set(e instanceof Error ? e.message : String(e));
  } finally {
    loading.set(false);
  }
}

/** Exit rewind mode and return to the live board. */
export async function returnToNow(): Promise<void> {
  rewindCommit.set(null);
  await loadBoard();
}

/** Replace a single ticket in the current board with a fresher copy (e.g. the
 *  ticket returned by a mutation), so the UI reflects the change immediately
 *  without waiting for the next poll. No-op if the ticket isn't on the board. */
export function applyTicket(next: Ticket): void {
  board.update((b) => {
    if (!b) return b;
    let changed = false;
    const lists = b.lists.map((l) => ({
      ...l,
      tickets: l.tickets.map((t) => {
        if (t.id === next.id) {
          changed = true;
          return next;
        }
        return t;
      })
    }));
    return changed ? { ...b, lists } : b;
  });
}

/** Move a ticket, then let the poll/WS reconcile. The move is a local action, so
 *  suppress the "just changed" flash for it (that highlight is for remote edits). */
export async function moveTicket(id: string, to: string, pos: number): Promise<void> {
  markSelfChange(id);
  try {
    await api.moveTicket(id, to, pos, project());
  } catch (e) {
    boardError.set(e instanceof Error ? e.message : String(e));
    await loadBoard(); // roll back to server truth
  }
}

/** Create a new list, then refresh. */
export async function createList(name: string): Promise<void> {
  try {
    await api.createList(name, project());
    await loadBoard();
  } catch (e) {
    boardError.set(e instanceof Error ? e.message : String(e));
  }
}

/** Rename a list, then refresh. */
export async function renameList(id: string, name: string): Promise<void> {
  try {
    await api.renameList(id, name, project());
    await loadBoard();
  } catch (e) {
    boardError.set(e instanceof Error ? e.message : String(e));
  }
}

/** Reorder a list to a target index, then refresh. */
export async function moveList(id: string, index: number): Promise<void> {
  try {
    await api.moveList(id, index, project());
    await loadBoard();
  } catch (e) {
    boardError.set(e instanceof Error ? e.message : String(e));
  }
}

/** Delete an (empty) list, then refresh. */
export async function deleteList(id: string): Promise<void> {
  try {
    await api.deleteList(id, true, project());
    await loadBoard();
  } catch (e) {
    boardError.set(e instanceof Error ? e.message : String(e));
  }
}

/** (Re)initialise everything for the current environment. */
export async function bootstrap(): Promise<void> {
  const ok = await checkHealth();
  if (!ok) return;
  // Honor user-global UI styling (accent/theme) unless overridden locally.
  try {
    applyServerDefaults(await api.appConfig());
  } catch {
    /* styling defaults are non-critical */
  }
  await loadProjects();
  await Promise.all([loadBoard(), loadDefinitions(), loadIdentities(), loadGraph()]);
  startLiveUpdates();
  startPolling();
}

/** Reload everything for the current project (after a project switch). */
export async function reloadProject(): Promise<void> {
  rewindCommit.set(null);
  resetForumIndicator();
  clearChanges();
  // Forget the previous project's provenance so its snapshot isn't diffed against
  // the new board (which would flash every card). The old board stays on screen
  // until the new one loads, so there's no loading flash on switch.
  boardProject = null;
  await Promise.all([loadBoard(), loadDefinitions(), loadIdentities(), loadGraph()]);
  startPolling();
}

/** Subscribe to WS change events; refetch the live board (not while rewinding). */
export function startLiveUpdates(): void {
  stopLiveUpdates();
  unsubscribeWs = subscribeChanges(() => {
    if (get(rewindCommit)) return;
    void loadBoard({ silent: true });
    void loadDefinitions();
    void loadIdentities();
    void loadGraph();
  });
}

export function stopLiveUpdates(): void {
  unsubscribeWs?.();
  unsubscribeWs = null;
  stopPolling();
  clearChanges();
}

// --- live polling -----------------------------------------------------------

let pollTimer: ReturnType<typeof setInterval> | null = null;
let forumTick = 0;
let lastForumPosts = -1;
let onForumView = false;

/** Poll the active board every 0.5s so agent-driven changes appear (and animate)
 *  even if a WS event is missed. Idempotent; safe to call repeatedly. */
export function startPolling(): void {
  stopPolling();
  pollTimer = setInterval(() => {
    if (get(rewindCommit)) return; // frozen while viewing history
    void loadBoard({ silent: true });
    // Check the forum a little less often (every ~2s) for the unread indicator.
    forumTick = (forumTick + 1) % 4;
    if (forumTick === 0) void refreshForumIndicator();
  }, 500);
}

export function stopPolling(): void {
  if (pollTimer) {
    clearInterval(pollTimer);
    pollTimer = null;
  }
}

/** Tell the store whether the forum view is currently open (clears the unread
 *  indicator and re-baselines while it's open). */
export function setForumView(active: boolean): void {
  onForumView = active;
  if (active) forumUnread.set(false);
}

/** Reset the forum unread baseline (on project switch). */
export function resetForumIndicator(): void {
  lastForumPosts = -1;
  forumUnread.set(false);
}

async function refreshForumIndicator(): Promise<void> {
  try {
    const threads = await api.forumThreads(project());
    const total = threads.reduce((n, t) => n + (t.posts ?? 0), 0);
    if (lastForumPosts >= 0 && total > lastForumPosts && !onForumView) forumUnread.set(true);
    lastForumPosts = total;
  } catch {
    /* forum indicator is best-effort */
  }
}
