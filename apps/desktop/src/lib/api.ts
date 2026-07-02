// Small typed client for the wipe-daemon REST + WebSocket API.
import { browser } from '$app/environment';
import type {
  Attachment,
  Board,
  CreateTicketInput,
  Definitions,
  GraphCommit,
  Health,
  Identity,
  IdentityKind,
  LabelDef,
  List,
  Project,
  Ticket,
  TicketPatch
} from './types';

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
  // When served by the daemon itself (any origin other than the Vite dev
  // server on :5173), use that same origin so `wipe serve --port <any>` works.
  if (browser && window.location.port !== '5173') {
    return window.location.origin.replace(/\/$/, '');
  }
  return DEFAULT_BASE;
}

/** Persist a user override for the API base URL (empty string clears it). */
export function setApiBase(url: string): void {
  if (!browser) return;
  const trimmed = url.trim().replace(/\/$/, '');
  if (trimmed) window.localStorage.setItem(STORAGE_KEY, trimmed);
  else window.localStorage.removeItem(STORAGE_KEY);
}

function qs(params: Record<string, string | number | undefined>): string {
  const parts = Object.entries(params)
    .filter(([, v]) => v !== undefined && v !== '')
    .map(([k, v]) => `${k}=${encodeURIComponent(String(v))}`);
  return parts.length ? `?${parts.join('&')}` : '';
}

async function parseError(res: Response): Promise<string> {
  let msg = `${res.status} ${res.statusText}`;
  try {
    const j = await res.json();
    if (j?.error) msg = j.error;
  } catch {
    /* ignore body parse errors */
  }
  return msg;
}

async function req<T>(path: string, init?: RequestInit): Promise<T> {
  const res = await fetch(`${getApiBase()}${path}`, {
    ...init,
    headers: { 'content-type': 'application/json', ...(init?.headers ?? {}) }
  });
  if (!res.ok) throw new Error(await parseError(res));
  return (await res.json()) as T;
}

/**
 * The daemon omits empty arrays from ticket JSON (for clean on-disk diffs), so
 * they arrive as `undefined`. Normalize every ticket to always have its arrays.
 */
function fillTicket(t: Ticket): Ticket {
  return {
    ...t,
    body: t.body ?? '',
    labels: t.labels ?? [],
    assignees: t.assignees ?? [],
    comments: t.comments ?? [],
    attachments: t.attachments ?? []
  };
}

function fillBoard(b: Board): Board {
  return {
    ...b,
    lists: (b.lists ?? []).map((l) => ({ ...l, tickets: (l.tickets ?? []).map(fillTicket) }))
  };
}

/** Build a media URL, preserving path slashes but encoding each segment. */
export function mediaUrl(path: string, project?: string): string {
  const encoded = path
    .split('/')
    .map((seg) => encodeURIComponent(seg))
    .join('/');
  return `${getApiBase()}/api/media/${encoded}${qs({ project })}`;
}

export const api = {
  health(): Promise<Health> {
    return req<Health>('/api/health');
  },

  async projects(): Promise<Project[]> {
    const r = await req<{ projects: Project[] }>('/api/projects');
    return r.projects ?? [];
  },

  async board(project?: string): Promise<Board> {
    return fillBoard(await req<Board>(`/api/board${qs({ project })}`));
  },

  async graph(project?: string): Promise<GraphCommit[]> {
    const r = await req<{ commits: GraphCommit[] }>(`/api/graph${qs({ project })}`);
    return (r.commits ?? []).map((c) => ({
      ...c,
      parents: c.parents ?? [],
      refs: c.refs ?? []
    }));
  },

  async boardAt(commit: string, project?: string): Promise<Board> {
    return fillBoard(await req<Board>(`/api/board/at${qs({ commit, project })}`));
  },

  definitions(project?: string): Promise<Definitions> {
    return req<Definitions>(`/api/definitions${qs({ project })}`);
  },

  async identities(project?: string): Promise<Identity[]> {
    const r = await req<{ identities: Identity[] }>(`/api/identities${qs({ project })}`);
    return r.identities ?? [];
  },

  putIdentity(
    id: string,
    display_name: string,
    kind?: IdentityKind,
    project?: string
  ): Promise<Identity> {
    return req<Identity>(`/api/identities/${encodeURIComponent(id)}${qs({ project })}`, {
      method: 'PUT',
      body: JSON.stringify({ display_name, kind })
    });
  },

  createLabel(name: string, color?: string, project?: string): Promise<LabelDef> {
    return req<LabelDef>(`/api/labels${qs({ project })}`, {
      method: 'POST',
      body: JSON.stringify({ name, color })
    });
  },

  recolorLabel(name: string, color: string, project?: string): Promise<LabelDef> {
    return req<LabelDef>(`/api/labels/${encodeURIComponent(name)}${qs({ project })}`, {
      method: 'PATCH',
      body: JSON.stringify({ color })
    });
  },

  deleteLabel(name: string, project?: string): Promise<{ ok: boolean }> {
    return req<{ ok: boolean }>(`/api/labels/${encodeURIComponent(name)}${qs({ project })}`, {
      method: 'DELETE'
    });
  },

  createList(name: string, project?: string): Promise<List> {
    return req<List>(`/api/lists${qs({ project })}`, {
      method: 'POST',
      body: JSON.stringify({ name })
    });
  },

  renameList(id: string, name: string, project?: string): Promise<{ ok: boolean }> {
    return req<{ ok: boolean }>(`/api/lists/${encodeURIComponent(id)}${qs({ project })}`, {
      method: 'PATCH',
      body: JSON.stringify({ name })
    });
  },

  moveList(id: string, index: number, project?: string): Promise<{ ok: boolean }> {
    return req<{ ok: boolean }>(`/api/lists/${encodeURIComponent(id)}/move${qs({ project })}`, {
      method: 'POST',
      body: JSON.stringify({ index })
    });
  },

  deleteList(id: string, force = false, project?: string): Promise<{ ok: boolean }> {
    return req<{ ok: boolean }>(
      `/api/lists/${encodeURIComponent(id)}${qs({ project, force: force ? 'true' : undefined })}`,
      { method: 'DELETE' }
    );
  },

  async createTicket(input: CreateTicketInput, project?: string): Promise<Ticket> {
    return fillTicket(
      await req<Ticket>(`/api/tickets${qs({ project })}`, {
        method: 'POST',
        body: JSON.stringify(input)
      })
    );
  },

  async patchTicket(id: string, patch: TicketPatch, project?: string): Promise<Ticket> {
    return fillTicket(
      await req<Ticket>(`/api/tickets/${encodeURIComponent(id)}${qs({ project })}`, {
        method: 'PATCH',
        body: JSON.stringify(patch)
      })
    );
  },

  moveTicket(id: string, to: string, pos?: number, project?: string): Promise<{ ok: boolean }> {
    return req<{ ok: boolean }>(`/api/tickets/${encodeURIComponent(id)}/move${qs({ project })}`, {
      method: 'POST',
      body: JSON.stringify({ to, pos })
    });
  },

  addComment(
    id: string,
    body: string,
    author?: string,
    project?: string
  ): Promise<{ ok: boolean; comment: string }> {
    return req<{ ok: boolean; comment: string }>(
      `/api/tickets/${encodeURIComponent(id)}/comments${qs({ project })}`,
      { method: 'POST', body: JSON.stringify({ body, author }) }
    );
  },

  async uploadAttachment(id: string, file: File, project?: string): Promise<Attachment> {
    const form = new FormData();
    form.append('file', file);
    const res = await fetch(
      `${getApiBase()}/api/tickets/${encodeURIComponent(id)}/attachments${qs({ project })}`,
      { method: 'POST', body: form }
    );
    if (!res.ok) throw new Error(await parseError(res));
    return (await res.json()) as Attachment;
  },

  deleteAttachment(id: string, path: string, project?: string): Promise<{ ok: boolean }> {
    return req<{ ok: boolean }>(
      `/api/tickets/${encodeURIComponent(id)}/attachments${qs({ project })}`,
      { method: 'DELETE', body: JSON.stringify({ path }) }
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

  const wsUrl = () => getApiBase().replace(/^http/, 'ws') + '/ws';

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
