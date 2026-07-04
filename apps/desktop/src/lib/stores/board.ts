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

export const definitions = writable<Definitions>({
  version: 0,
  labels: [],
  priorities: []
});
export const identities = writable<Identity[]>([]);

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
      // Prefer the board `wipe serve` was launched in; otherwise fall back to the
      // first registered board.
      const pick = served && list.some((p) => p.path === served) ? served : (list[0]?.path ?? null);
      if (pick) currentProject.set(pick);
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
}

/** Reload everything for the current project (after a project switch). */
export async function reloadProject(): Promise<void> {
  rewindCommit.set(null);
  await Promise.all([loadBoard(), loadDefinitions(), loadIdentities(), loadGraph()]);
}

/** Subscribe to WS change events; refetch the live board (not while rewinding). */
export function startLiveUpdates(): void {
  stopLiveUpdates();
  unsubscribeWs = subscribeChanges(() => {
    if (get(rewindCommit)) return;
    void loadBoard();
    void loadDefinitions();
    void loadIdentities();
    void loadGraph();
  });
}

export function stopLiveUpdates(): void {
  unsubscribeWs?.();
  unsubscribeWs = null;
}
