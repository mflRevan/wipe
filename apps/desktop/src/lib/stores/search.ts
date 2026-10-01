// Board search (Ctrl/Cmd+F) and ticket quick-jump (type "T" + id digits).
//
// The search index is derived once per board snapshot: one normalized haystack
// per ticket covering everything a person might remember about it - id, title,
// description, labels, priority, people (ids and display names), comments and
// their authors, checklist and acceptance items, attachment names. Each keystroke
// is then a handful of substring scans over pre-lowered strings - no regex, no
// re-walking the board - so filtering stays instant on large boards.
import { derived, writable } from 'svelte/store';
import { board, identities } from './board';
import type { Board, Identity, Ticket } from '$lib/types';

/** The live search text; `null` when the search bar is closed. */
export const searchQuery = writable<string | null>(null);

/** Lower-case, accent-folded form used on both sides of every comparison. */
export function fold(s: string): string {
  return s.normalize('NFD').replace(/\p{Diacritic}/gu, '').toLowerCase();
}

/** Split a query into terms: whitespace-separated words, or "quoted phrases". */
export function terms(q: string | null): string[] {
  if (!q) return [];
  const out: string[] = [];
  for (const m of q.matchAll(/"([^"]+)"|(\S+)/g)) {
    const t = fold((m[1] ?? m[2] ?? '').trim());
    if (t) out.push(t);
  }
  return out;
}

type Entry = { id: string; hay: string; title: string; labels: string };

function displayNames(ids: string[], people: Map<string, string>): string {
  return ids
    .map((id) => {
      const email = id.match(/<([^>]+)>/)?.[1];
      return `${id} ${people.get(id) ?? (email ? (people.get(email) ?? '') : '')}`;
    })
    .join(' ');
}

function entryFor(t: Ticket, people: Map<string, string>): Entry {
  const parts = [
    t.id,
    t.id.replace('-', ''),
    t.legacy_id ?? '',
    t.title,
    t.body,
    t.priority ?? '',
    t.labels.join(' '),
    displayNames(t.assignees, people),
    displayNames([...new Set(t.comments.map((c) => c.author))], people),
    displayNames([...new Set(t.activity.map((a) => a.actor))], people),
    t.comments.map((c) => c.body).join('\n'),
    t.checklist.map((i) => i.text).join('\n'),
    t.acceptance.map((i) => i.text).join('\n'),
    t.attachments.map((a) => a.name).join(' ')
  ];
  return {
    id: t.id,
    hay: fold(parts.join('\n')),
    title: fold(t.title),
    labels: fold(t.labels.join(' '))
  };
}

/** The per-board index, rebuilt only when the board or identities change. Ticket
 *  objects that survive a refresh unchanged keep their entry (cheap rebuilds). */
let cache = new WeakMap<Ticket, Entry>();
let cachedIds: unknown = null;
export const searchIndex = derived([board, identities], ([$board, $ids]) => {
  // Display names feed the index; new identities invalidate every entry.
  if ($ids !== cachedIds) {
    cache = new WeakMap();
    cachedIds = $ids;
  }
  const people = new Map<string, string>(($ids as Identity[]).map((i) => [i.id, i.display_name]));
  const map = new Map<string, Entry>();
  for (const l of ($board as Board | null)?.lists ?? []) {
    for (const t of l.tickets) {
      let e = cache.get(t);
      if (!e) {
        e = entryFor(t, people);
        cache.set(t, e);
      }
      map.set(t.id, e);
    }
  }
  return map;
});

/** Where a ticket matched: drives highlighting and the "matched in" hint. */
export type Match = { title: boolean; labels: boolean; elsewhere: boolean };

/** Ticket id -> how it matched, for every ticket matching ALL terms. `null` when
 *  no search is active (everything visible). */
export const searchMatches = derived([searchIndex, searchQuery], ([$index, $q]) => {
  const ts = terms($q);
  if (!ts.length) return null;
  const out = new Map<string, Match>();
  for (const [id, e] of $index) {
    if (!ts.every((t) => e.hay.includes(t))) continue;
    const title = ts.some((t) => e.title.includes(t));
    const labels = ts.some((t) => e.labels.includes(t));
    out.set(id, { title, labels, elsewhere: !title && !labels });
  }
  return out;
});

/** Split `text` into plain/marked segments for every occurrence of any term. */
export function highlight(text: string, ts: string[]): { s: string; hit: boolean }[] {
  if (!ts.length || !text) return [{ s: text, hit: false }];
  const f = fold(text);
  // Folding can change length (rare: some ligatures); fall back to no marks then.
  if (f.length !== text.length) return [{ s: text, hit: false }];
  const marks = new Array<boolean>(text.length).fill(false);
  for (const t of ts) {
    let i = f.indexOf(t);
    while (i !== -1) {
      for (let k = i; k < i + t.length; k++) marks[k] = true;
      i = f.indexOf(t, i + t.length);
    }
  }
  const out: { s: string; hit: boolean }[] = [];
  for (let i = 0; i < text.length; i++) {
    const last = out[out.length - 1];
    if (last && last.hit === marks[i]) last.s += text[i];
    else out.push({ s: text[i], hit: marks[i] });
  }
  return out;
}

// --- quick-jump --------------------------------------------------------------

/** The digits typed after "T" (e.g. "2A"); `null` when no jump is in progress. */
export const jumpQuery = writable<string | null>(null);

/** The ticket the current quick-jump points at (its card glows). */
export const jumpTarget = writable<string | null>(null);

/** Resolve typed id digits to a ticket on the board, mirroring the CLI: on a hex
 *  board a short all-decimal entry first matches a pre-translation `legacy_id`,
 *  otherwise digits are hex (`T5` -> `T-005`); legacy boards are decimal. */
export function resolveJump(b: Board | null, digits: string): Ticket | null {
  if (!b || !digits) return null;
  const all = b.lists.flatMap((l) => l.tickets);
  const hex = b.ids === 'hex';
  const valueOf = (id: string, radix: number) => {
    const n = parseInt(id.replace(/^T-/, ''), radix);
    return Number.isNaN(n) ? null : n;
  };
  const byLegacy = () => {
    const n = parseInt(digits, 10);
    return all.find((t) => t.legacy_id && valueOf(t.legacy_id, 10) === n) ?? null;
  };
  if (!hex) {
    if (!/^\d+$/.test(digits)) return null;
    const n = parseInt(digits, 10);
    return all.find((t) => valueOf(t.id, 10) === n) ?? null;
  }
  const byHex = () => {
    const n = parseInt(digits, 16);
    return all.find((t) => valueOf(t.id, 16) === n) ?? null;
  };
  const legacyFirst = digits.length < 3 && /^\d+$/.test(digits);
  return legacyFirst ? (byLegacy() ?? byHex()) : (byHex() ?? byLegacy());
}
