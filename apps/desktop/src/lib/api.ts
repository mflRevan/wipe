// Small typed client for the wipe-daemon REST + WebSocket API.
import { browser } from '$app/environment';
import type {
  AppConfig,
  Attachment,
  Board,
  ChecksKind,
  CreateTicketInput,
  Definitions,
  ForumMatch,
  ForumPost,
  ForumThread,
  ForumThreadSummary,
  GraphCommit,
  Health,
  Identity,
  IdentityKind,
  LabelDef,
  List,
  Project,
  Ticket,
  TicketPatch,
  TrashListResponse
} from './types';

const DEFAULT_BASE = 'http://localhost:6737';
const STORAGE_KEY = 'wipe.apiBase';
const TOKEN_KEY = 'wipe.token';

/**
 * Resolve the access token for an exposed daemon. On first load the daemon hands
 * it out in the URL (`?token=...`); we capture it into localStorage and strip it
 * from the address bar so it isn't left lying around or copied by accident.
 */
export function getToken(): string {
  if (!browser) return '';
  try {
    const url = new URL(window.location.href);
    const fromUrl = url.searchParams.get('token');
    if (fromUrl && fromUrl.trim()) {
      window.localStorage.setItem(TOKEN_KEY, fromUrl.trim());
      url.searchParams.delete('token');
      window.history.replaceState({}, '', url.toString());
      return fromUrl.trim();
    }
    return window.localStorage.getItem(TOKEN_KEY)?.trim() ?? '';
  } catch {
    return '';
  }
}

/** Authorization header for the token, or an empty object when there is none. */
function authHeader(): Record<string, string> {
  const t = getToken();
  return t ? { authorization: `Bearer ${t}` } : {};
}

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
    headers: { 'content-type': 'application/json', ...authHeader(), ...(init?.headers ?? {}) }
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
    attachments: t.attachments ?? [],
    checklist: t.checklist ?? [],
    acceptance: t.acceptance ?? [],
    activity: t.activity ?? []
  };
}

function fillBoard(b: Board): Board {
  return {
    ...b,
    lists: (b.lists ?? []).map((l) => ({ ...l, tickets: (l.tickets ?? []).map(fillTicket) }))
  };
}

/** The daemon omits empty arrays; a leaf post arrives with no `replies`. Restore
 *  every array so the tree is safe to walk. */
function fillPost(p: ForumPost): ForumPost {
  return {
    ...p,
    labels: p.labels ?? [],
    refs: p.refs ?? [],
    attachments: p.attachments ?? [],
    replies: (p.replies ?? []).map(fillPost)
  };
}

function fillThread(t: ForumThread): ForumThread {
  return { ...t, root: fillPost(t.root) };
}

/** Build a media URL, preserving path slashes but encoding each segment. */
export function mediaUrl(path: string, project?: string): string {
  const encoded = path
    .split('/')
    .map((seg) => encodeURIComponent(seg))
    .join('/');
  // Media loads via <img src>, which can't set an Authorization header, so the
  // token (when present, i.e. exposed mode) rides along as a query parameter.
  return `${getApiBase()}/api/media/${encoded}${qs({ project, token: getToken() || undefined })}`;
}

export const api = {
  health(): Promise<Health> {
    return req<Health>('/api/health');
  },

  /** User-global defaults (styling + default identity) set via `wipe config --global`. */
  appConfig(): Promise<AppConfig> {
    return req<AppConfig>('/api/config');
  },

  /** Update user-global preferences (styling + default identity). */
  patchConfig(patch: Partial<AppConfig>): Promise<AppConfig> {
    return req<AppConfig>('/api/config', { method: 'PATCH', body: JSON.stringify(patch) });
  },

  /** Rescan the disk for boards and refresh the registry. */
  rescan(): Promise<{ found: number; projects: Project[] }> {
    return req<{ found: number; projects: Project[] }>('/api/scan', { method: 'POST' });
  },

  /** Registered boards plus `current` - the board the daemon was launched in
   *  (null when served globally), so the UI can default-open it. */
  async projects(): Promise<{ projects: Project[]; current: string | null }> {
    const r = await req<{ projects: Project[]; current?: string | null }>('/api/projects');
    return { projects: r.projects ?? [], current: r.current ?? null };
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

  deleteIdentity(id: string, project?: string): Promise<{ ok: boolean }> {
    return req<{ ok: boolean }>(`/api/identities/${encodeURIComponent(id)}${qs({ project })}`, {
      method: 'DELETE'
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

  deleteTicket(id: string, project?: string, purge = false): Promise<{ ok: boolean }> {
    return req<{ ok: boolean }>(
      `/api/tickets/${encodeURIComponent(id)}${qs({ project, purge: purge ? 'true' : undefined })}`,
      { method: 'DELETE' }
    );
  },

  async duplicateTicket(id: string, project?: string): Promise<Ticket> {
    return fillTicket(
      await req<Ticket>(`/api/tickets/${encodeURIComponent(id)}/duplicate${qs({ project })}`, {
        method: 'POST'
      })
    );
  },

  // --- trash (soft-deleted tickets) ----------------------------------------

  trashList(project?: string): Promise<TrashListResponse> {
    return req<TrashListResponse>(`/api/trash${qs({ project })}`);
  },

  async restoreTicket(id: string, project?: string): Promise<Ticket> {
    return fillTicket(
      await req<Ticket>(`/api/trash/${encodeURIComponent(id)}/restore${qs({ project })}`, {
        method: 'POST'
      })
    );
  },

  purgeTrash(id: string, project?: string): Promise<{ ok: boolean; purged: boolean }> {
    return req<{ ok: boolean; purged: boolean }>(`/api/trash/${encodeURIComponent(id)}${qs({ project })}`, {
      method: 'DELETE'
    });
  },

  emptyTrash(project?: string): Promise<{ ok: boolean; purged: number }> {
    return req<{ ok: boolean; purged: number }>(`/api/trash${qs({ project })}`, {
      method: 'DELETE'
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

  async deleteComment(id: string, comment: string, project?: string): Promise<Ticket> {
    return fillTicket(
      await req<Ticket>(
        `/api/tickets/${encodeURIComponent(id)}/comments/${encodeURIComponent(comment)}${qs({ project })}`,
        { method: 'DELETE' }
      )
    );
  },

  // --- checklist & acceptance criteria -------------------------------------
  // `kind` picks the surface ('checklist' | 'acceptance'); the two share routes
  // shapes. Each mutation returns the full updated ticket, so callers can
  // re-render immediately without waiting for the next poll.

  async addCheckItem(kind: ChecksKind, id: string, text: string, project?: string): Promise<Ticket> {
    return fillTicket(
      await req<Ticket>(`/api/tickets/${encodeURIComponent(id)}/${kind}${qs({ project })}`, {
        method: 'POST',
        body: JSON.stringify({ text })
      })
    );
  },

  async setCheckItem(
    kind: ChecksKind,
    id: string,
    item: string,
    patch: { done?: boolean; text?: string },
    project?: string
  ): Promise<Ticket> {
    return fillTicket(
      await req<Ticket>(
        `/api/tickets/${encodeURIComponent(id)}/${kind}/${encodeURIComponent(item)}${qs({ project })}`,
        { method: 'PATCH', body: JSON.stringify(patch) }
      )
    );
  },

  async removeCheckItem(kind: ChecksKind, id: string, item: string, project?: string): Promise<Ticket> {
    return fillTicket(
      await req<Ticket>(
        `/api/tickets/${encodeURIComponent(id)}/${kind}/${encodeURIComponent(item)}${qs({ project })}`,
        { method: 'DELETE' }
      )
    );
  },

  async moveCheckItem(
    kind: ChecksKind,
    id: string,
    item: string,
    index: number,
    project?: string
  ): Promise<Ticket> {
    return fillTicket(
      await req<Ticket>(
        `/api/tickets/${encodeURIComponent(id)}/${kind}/${encodeURIComponent(item)}/move${qs({ project })}`,
        { method: 'POST', body: JSON.stringify({ index }) }
      )
    );
  },

  async uploadAttachment(id: string, file: File, project?: string): Promise<Attachment> {
    const form = new FormData();
    form.append('file', file);
    const res = await fetch(
      `${getApiBase()}/api/tickets/${encodeURIComponent(id)}/attachments${qs({ project })}`,
      { method: 'POST', body: form, headers: { ...authHeader() } }
    );
    if (!res.ok) throw new Error(await parseError(res));
    return (await res.json()) as Attachment;
  },

  attachPath(id: string, path: string, project?: string): Promise<Attachment> {
    return req<Attachment>(
      `/api/tickets/${encodeURIComponent(id)}/attachments/path${qs({ project })}`,
      { method: 'POST', body: JSON.stringify({ path }) }
    );
  },

  deleteAttachment(id: string, path: string, project?: string): Promise<{ ok: boolean }> {
    return req<{ ok: boolean }>(
      `/api/tickets/${encodeURIComponent(id)}/attachments${qs({ project })}`,
      { method: 'DELETE', body: JSON.stringify({ path }) }
    );
  },

  // --- forum ---------------------------------------------------------------

  async forumThreads(project?: string): Promise<ForumThreadSummary[]> {
    const r = await req<{ threads: ForumThreadSummary[] }>(`/api/forum${qs({ project })}`);
    return r.threads ?? [];
  },

  async forumThread(id: string, project?: string): Promise<ForumThread> {
    return fillThread(await req<ForumThread>(`/api/forum/${encodeURIComponent(id)}${qs({ project })}`));
  },

  async forumPost(
    input: { title: string; body?: string; labels?: string[] },
    project?: string
  ): Promise<ForumThread> {
    return fillThread(
      await req<ForumThread>(`/api/forum${qs({ project })}`, {
        method: 'POST',
        body: JSON.stringify(input)
      })
    );
  },

  forumReply(
    id: string,
    input: { body: string; labels?: string[] },
    project?: string
  ): Promise<{ ok: boolean; id: string; parent: string }> {
    return req(`/api/forum/${encodeURIComponent(id)}/reply${qs({ project })}`, {
      method: 'POST',
      body: JSON.stringify(input)
    });
  },

  forumEdit(id: string, body: string, project?: string): Promise<{ ok: boolean }> {
    return req(`/api/forum/${encodeURIComponent(id)}${qs({ project })}`, {
      method: 'PATCH',
      body: JSON.stringify({ body })
    });
  },

  forumDelete(id: string, project?: string): Promise<{ ok: boolean }> {
    return req(`/api/forum/${encodeURIComponent(id)}${qs({ project })}`, { method: 'DELETE' });
  },

  async forumSearch(
    params: { q?: string; author?: string; label?: string; scope?: string; titles?: boolean },
    project?: string
  ): Promise<ForumMatch[]> {
    const r = await req<{ posts: ForumMatch[] }>(
      `/api/forum/search${qs({
        project,
        q: params.q,
        author: params.author,
        label: params.label,
        scope: params.scope,
        titles: params.titles ? 'true' : undefined
      })}`
    );
    return r.posts ?? [];
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

  // Browser WebSockets can't set headers, so the token (exposed mode) goes on the
  // query string, matching the daemon's `?token=` acceptance.
  const wsUrl = () => {
    const base = getApiBase().replace(/^http/, 'ws') + '/ws';
    const t = getToken();
    return t ? `${base}?token=${encodeURIComponent(t)}` : base;
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
