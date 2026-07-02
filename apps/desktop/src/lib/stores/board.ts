import { writable, derived, get } from 'svelte/store';
import { api, subscribeChanges } from '$lib/api';
import type { Board, CommitInfo, Health, Project } from '$lib/types';

export const health = writable<Health | null>(null);
export const healthError = writable<string | null>(null);

export const projects = writable<Project[]>([]);
export const currentProject = writable<string | null>(null);

export const board = writable<Board | null>(null);
export const boardError = writable<string | null>(null);
export const loading = writable<boolean>(false);

// Rewind / time-machine state.
export const history = writable<CommitInfo[]>([]);
export const rewindCommit = writable<CommitInfo | null>(null);
export const rewinding = derived(rewindCommit, ($c) => $c !== null);

let unsubscribeWs: (() => void) | null = null;

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
    if (!cur && list.length > 0) {
      currentProject.set(list[0].path);
    } else if (cur && !list.some((p) => p.path === cur) && list.length > 0) {
      currentProject.set(list[0].path);
    }
  } catch (e) {
    boardError.set(e instanceof Error ? e.message : String(e));
  }
}

/** Fetch the live board for the current project. */
export async function loadBoard(): Promise<void> {
  const project = get(currentProject) ?? undefined;
  loading.set(true);
  try {
    const b = await api.board(project);
    board.set(b);
    boardError.set(null);
  } catch (e) {
    boardError.set(e instanceof Error ? e.message : String(e));
  } finally {
    loading.set(false);
  }
}

/** Fetch the commit history for the current project. */
export async function loadHistory(): Promise<void> {
  const project = get(currentProject) ?? undefined;
  try {
    history.set(await api.history(project));
  } catch (e) {
    boardError.set(e instanceof Error ? e.message : String(e));
  }
}

/** Enter rewind mode at a specific commit and load the historical snapshot. */
export async function enterRewind(commit: CommitInfo): Promise<void> {
  const project = get(currentProject) ?? undefined;
  loading.set(true);
  try {
    const b = await api.boardAt(commit.hash, project);
    board.set(b);
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

/** (Re)initialise everything for the current environment. */
export async function bootstrap(): Promise<void> {
  const ok = await checkHealth();
  if (!ok) return;
  await loadProjects();
  await loadBoard();
  await loadHistory();
  startLiveUpdates();
}

/** Subscribe to WS change events; refetch the live board (not while rewinding). */
export function startLiveUpdates(): void {
  stopLiveUpdates();
  unsubscribeWs = subscribeChanges(() => {
    if (get(rewindCommit)) return;
    void loadBoard();
    void loadHistory();
  });
}

export function stopLiveUpdates(): void {
  unsubscribeWs?.();
  unsubscribeWs = null;
}
