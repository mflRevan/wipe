import { marked } from 'marked';
import type { LabelDef } from './types';

/** The fixed, harmonious label/tag color set from DESIGN.md §4. */
export const LABEL_COLORS: Record<string, string> = {
  terracotta: '#CC785C',
  kraft: '#D4A27F',
  manilla: '#EBDBBC',
  sky: '#61AAF2',
  clay: '#BF4D43',
  sage: '#7E9B7A',
  indigo: '#6C7BA8',
  plum: '#9A7AA0',
  slate: '#666663'
};

export const LABEL_KEYS = Object.keys(LABEL_COLORS);

function hash(seed: string): number {
  let h = 0;
  for (let i = 0; i < seed.length; i++) h = (h * 31 + seed.charCodeAt(i)) >>> 0;
  return h;
}

/**
 * Resolve a label's chip color to a hex. Accepts a stored color key (preferred),
 * a raw hex (legacy/seed data), or falls back to a deterministic key by name.
 */
export function labelColor(name: string, color?: string): string {
  if (color) {
    if (LABEL_COLORS[color]) return LABEL_COLORS[color];
    if (/^#([0-9a-f]{3}|[0-9a-f]{6})$/i.test(color)) return color;
  }
  const key = LABEL_KEYS[hash(name) % LABEL_KEYS.length];
  return LABEL_COLORS[key];
}

export function labelColorFor(name: string, defs: LabelDef[]): string {
  const def = defs.find((d) => d.name === name);
  return labelColor(name, def?.color);
}

/** Deterministic avatar background color for an identity id. */
export function avatarColor(id: string): string {
  const key = LABEL_KEYS[hash(id) % LABEL_KEYS.length];
  return LABEL_COLORS[key];
}

/** Priority → { dot color, rank } for the card priority dot. */
export function priorityColor(p?: string): string {
  switch ((p ?? '').toLowerCase()) {
    case 'urgent':
      return '#BF4D43';
    case 'high':
      return '#CC785C';
    case 'medium':
      return '#61AAF2';
    case 'low':
      return '#91918D';
    default:
      return 'transparent';
  }
}

/** Initials for an avatar from a display name or id. */
export function initials(name: string): string {
  const cleaned = name.replace(/<[^>]*>/g, '').trim();
  const parts = cleaned.split(/[\s._-]+/).filter(Boolean);
  if (parts.length === 0) return '?';
  if (parts.length === 1) return parts[0].slice(0, 2).toUpperCase();
  return (parts[0][0] + parts[parts.length - 1][0]).toUpperCase();
}

/** Human-friendly timestamp. */
export function formatDate(iso: string): string {
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) return iso;
  return d.toLocaleString(undefined, {
    month: 'short',
    day: 'numeric',
    hour: '2-digit',
    minute: '2-digit'
  });
}

/**
 * Render an activity event as a human phrase. `resolveName` maps an identity id
 * to a display name (for assign/unassign events); defaults to the raw id.
 */
export function activityPhrase(
  kind: string,
  detail = '',
  resolveName: (id: string) => string = (id) => id
): string {
  switch (kind) {
    case 'created':
      return 'created this card';
    case 'moved':
      return `moved this to ${detail}`;
    case 'renamed':
      return `renamed this to “${detail}”`;
    case 'edited':
      return 'updated the description';
    case 'priority':
      return detail ? `set priority to ${detail}` : 'cleared the priority';
    case 'label-added':
      return `added label ${detail}`;
    case 'label-removed':
      return `removed label ${detail}`;
    case 'assigned':
      return `assigned ${resolveName(detail)}`;
    case 'unassigned':
      return `unassigned ${resolveName(detail)}`;
    case 'attached':
      return `attached ${detail}`;
    case 'detached':
      return `removed attachment ${detail}`;
    default:
      return kind;
  }
}

export function formatBytes(n: number): string {
  if (n < 1024) return `${n} B`;
  const units = ['KB', 'MB', 'GB'];
  let v = n / 1024;
  let i = 0;
  while (v >= 1024 && i < units.length - 1) {
    v /= 1024;
    i++;
  }
  return `${v.toFixed(v < 10 ? 1 : 0)} ${units[i]}`;
}

marked.setOptions({ gfm: true, breaks: true });

/** Render markdown to HTML (synchronous). */
export function renderMarkdown(src: string): string {
  if (!src) return '';
  return marked.parse(src, { async: false }) as string;
}

/** Categorize an attachment MIME for rendering (DESIGN.md §9). */
export type MediaKind = 'image' | 'audio' | 'video' | 'pdf' | 'text' | 'other';

export function mediaKind(mime: string, name: string): MediaKind {
  const m = (mime || '').toLowerCase();
  const ext = name.toLowerCase().split('.').pop() ?? '';
  if (m.startsWith('image/') || ['png', 'jpg', 'jpeg', 'gif', 'webp', 'svg'].includes(ext))
    return 'image';
  if (m.startsWith('audio/') || ['mp3', 'ogg', 'wav'].includes(ext)) return 'audio';
  if (m.startsWith('video/') || ['mp4', 'webm', 'mov'].includes(ext)) return 'video';
  if (m === 'application/pdf' || ext === 'pdf') return 'pdf';
  if (m.startsWith('text/') || ['md', 'txt', 'csv', 'log', 'json'].includes(ext)) return 'text';
  return 'other';
}
