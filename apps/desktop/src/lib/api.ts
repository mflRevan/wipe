// Small typed client for the wipe-daemon REST + WebSocket API.
import { browser } from '$app/environment';
import type { Board, CommitInfo, CreateTicketInput, Health, Project, Ticket } from './types';

const DEFAULT_BASE = 'http://localhost:6737';
const STORAGE_KEY = 'wipe.apiBase';

/** Resolve the configured API base URL. Order: localStorage > VITE_WIPE_API > default. */
export function getApiBase(): string {
  if (browser) {
    const stored = window.localStorage.getItem(STORAGE_KEY);
    if (stored && stored.trim()) return stored.trim().replace(/\/$/, '');
  }
  const env = import.meta.env.VITE_WIPE_API as string | undefined;
  if (env && env.trim()) return env.trim().replace(/\/$/, '');
  return DEFAULT_BASE;
}

/** Persist a user override for the API base URL (empty string clears it). */
export function setApiBase(url: string): void {
  if (!browser) return;
  const trimmed = url.trim().replace(/\/$/, '');
  if (trimmed) window.localStorage.setItem(STORAGE_KEY, trimmed);
  else window.localStorage.removeItem(STORAGE_KEY);
}

async function req<T>(path: string, init?: RequestInit): Promise<T> {
  const res = await fetch(`${getApiBase()}${path}`, {
    ...init,
    headers: { 'content-type': 'application/json', ...(init?.headers ?? {}) }
  });
  if (!res.ok) {
    let msg = `${res.status} ${res.statusText}`;
    try {
      const j = await res.json();
      if (j?.error) msg = j.error;
    } catch {
      /* ignore body parse errors */
    }
    throw new Error(msg);
  }
  return (await res.json()) as T;
}

export const api = {
  health(): Promise<Health> {
    return req<Health>('/api/health');
  },

  async projects(): Promise<Project[]> {
    const r = await req<{ projects: Project[] }>('/api/projects');
    return r.projects ?? [];
  },

  board(project?: string): Promise<Board> {
    const q = project ? `?project=${encodeURIComponent(project)}` : '';
    return req<Board>(`/api/board${q}`);
  },

  async history(project?: string): Promise<CommitInfo[]> {
    const q = project ? `?project=${encodeURIComponent(project)}` : '';
    const r = await req<{ commits: CommitInfo[] }>(`/api/history${q}`);
    return r.commits ?? [];
  },

  boardAt(commit: string, project?: string): Promise<Board> {
    const p = project ? `&project=${encodeURIComponent(project)}` : '';
    return req<Board>(`/api/board/at?commit=${encodeURIComponent(commit)}${p}`);
  },

  createTicket(input: CreateTicketInput, project?: string): Promise<Ticket> {
    return req<Ticket>('/api/tickets', {
      method: 'POST',
      body: JSON.stringify({ ...input, project })
    });
  },

  moveTicket(id: string, to: string, pos: number, project?: string): Promise<{ ok: boolean }> {
    return req<{ ok: boolean }>(`/api/tickets/${encodeURIComponent(id)}/move`, {
      method: 'POST',
      body: JSON.stringify({ to, pos, project })
    });
  },

  addComment(
    id: string,
    body: string,
    author?: string,
    project?: string
  ): Promise<{ ok: boolean; comment: string }> {
    return req<{ ok: boolean; comment: string }>(
      `/api/tickets/${encodeURIComponent(id)}/comments`,
      {
        method: 'POST',
        body: JSON.stringify({ body, author, project })
      }
    );
  }
};

/**
 * Subscribe to the daemon's change WebSocket. Calls `onChange` whenever the
 * board changes. Auto-reconnects with backoff. Returns an unsubscribe fn.
 */
export function subscribeChanges(onChange: () => void): () => void {
  if (!browser) return () => {};
  let socket: WebSocket | null = null;
  let closed = false;
  let retry = 0;
  let timer: ReturnType<typeof setTimeout> | undefined;

  const wsUrl = () => {
    const base = getApiBase();
    return base.replace(/^http/, 'ws') + '/ws';
  };

  const connect = () => {
    if (closed) return;
    try {
      socket = new WebSocket(wsUrl());
    } catch {
      scheduleReconnect();
      return;
    }
    socket.onmessage = (ev) => {
      if (typeof ev.data === 'string' && ev.data.trim() === 'changed') onChange();
    };
    socket.onopen = () => {
      retry = 0;
    };
    socket.onclose = () => scheduleReconnect();
    socket.onerror = () => socket?.close();
  };

  const scheduleReconnect = () => {
    if (closed) return;
    retry = Math.min(retry + 1, 6);
    timer = setTimeout(connect, 500 * 2 ** (retry - 1));
  };

  connect();

  return () => {
    closed = true;
    if (timer) clearTimeout(timer);
    socket?.close();
  };
}
