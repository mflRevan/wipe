import { writable, derived, get } from 'svelte/store';
import { api, subscribeChanges } from '$lib/api';
import type { Board, CommitInfo, Definitions, Health, Identity, Project, Ticket } from '$lib/types';

export const health = writable<Health | null>(null);
export const healthError = writable<string | null>(null);

export const projects = writable<Project[]>([]);
export const currentProject = writable<string | null>(null);

export const board = writable<Board | null>(null);
export const boardError = writable<string | null>(null);
export const loading = writable<boolean>(false);

export const definitions = writable<Definitions>({
  types: [],
  labels: [],
  tags: [],
  priorities: []
});
export const identities = writable<Identity[]>([]);

// Rewind / time-machine state.
export const history = writable<CommitInfo[]>([]);
export const rewindCommit = writable<CommitInfo | null>(null);
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
    const list = await api.projects();
    projects.set(list);
    const cur = get(currentProject);
    if ((!cur || !list.some((p) => p.path === cur)) && list.length > 0) {
      currentProject.set(list[0].path);
    }
  } catch (e) {
    boardError.set(e instanceof Error ? e.message : String(e));
  }
}

/** Fetch the live board for the current project. */
export async function loadBoard(): Promise<void> {
  loading.set(true);
  try {
    board.set(await api.board(project()));
    boardError.set(null);
  } catch (e) {
    boardError.set(e instanceof Error ? e.message : String(e));
  } finally {
    loading.set(false);
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

/** Fetch the commit history for the current project. */
export async function loadHistory(): Promise<void> {
  try {
    history.set(await api.history(project()));
  } catch (e) {
    boardError.set(e instanceof Error ? e.message : String(e));
  }
}

/** Enter rewind mode at a specific commit and load the historical snapshot. */
export async function enterRewind(commit: CommitInfo): Promise<void> {
  loading.set(true);
  try {
    board.set(await api.boardAt(commit.hash, project()));
    rewindCommit.set(commit);
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

/** Optimistically move a ticket, then persist + let WS reconcile. */
export async function moveTicket(id: string, to: string, pos: number): Promise<void> {
  try {
    await api.moveTicket(id, to, pos, project());
  } catch (e) {
    boardError.set(e instanceof Error ? e.message : String(e));
    await loadBoard(); // roll back to server truth
  }
}

/** (Re)initialise everything for the current environment. */
export async function bootstrap(): Promise<void> {
  const ok = await checkHealth();
  if (!ok) return;
  await loadProjects();
  await Promise.all([loadBoard(), loadDefinitions(), loadIdentities(), loadHistory()]);
  startLiveUpdates();
}

/** Reload everything for the current project (after a project switch). */
export async function reloadProject(): Promise<void> {
  rewindCommit.set(null);
  await Promise.all([loadBoard(), loadDefinitions(), loadIdentities(), loadHistory()]);
}

/** Subscribe to WS change events; refetch the live board (not while rewinding). */
export function startLiveUpdates(): void {
  stopLiveUpdates();
  unsubscribeWs = subscribeChanges(() => {
    if (get(rewindCommit)) return;
    void loadBoard();
    void loadDefinitions();
    void loadIdentities();
    void loadHistory();
  });
}

export function stopLiveUpdates(): void {
  unsubscribeWs?.();
  unsubscribeWs = null;
}
