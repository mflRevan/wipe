import { writable } from 'svelte/store';
import { browser } from '$app/environment';

export type Appearance = 'light' | 'dark' | 'system';
export type Accent = 'book-cloth' | 'kraft' | 'focus' | 'sage';

const THEME_KEY = 'wipe.theme';
const ACCENT_KEY = 'wipe.accent';

export const ACCENTS: { id: Accent; name: string; hex: string }[] = [
  { id: 'book-cloth', name: 'Book Cloth', hex: '#CC785C' },
  { id: 'kraft', name: 'Kraft', hex: '#D4A27F' },
  { id: 'focus', name: 'Focus', hex: '#61AAF2' },
  { id: 'sage', name: 'Sage', hex: '#7E9B7A' }
];

function initialAppearance(): Appearance {
  if (!browser) return 'system';
  const v = localStorage.getItem(THEME_KEY);
  return v === 'light' || v === 'dark' || v === 'system' ? v : 'system';
}

function initialAccent(): Accent {
  if (!browser) return 'book-cloth';
  const v = localStorage.getItem(ACCENT_KEY) as Accent | null;
  return v && ACCENTS.some((a) => a.id === v) ? v : 'book-cloth';
}

export const appearance = writable<Appearance>(initialAppearance());
export const accent = writable<Accent>(initialAccent());

function systemDark(): boolean {
  return browser && matchMedia('(prefers-color-scheme: dark)').matches;
}

function applyAppearance(a: Appearance): void {
  if (!browser) return;
  const dark = a === 'dark' || (a === 'system' && systemDark());
  document.documentElement.setAttribute('data-theme', dark ? 'dark' : 'light');
}

function applyAccent(a: Accent): void {
  if (!browser) return;
  const root = document.documentElement;
  if (a === 'book-cloth') root.removeAttribute('data-accent');
  else root.setAttribute('data-accent', a);
}

export function setAppearance(a: Appearance): void {
  appearance.set(a);
  if (browser) localStorage.setItem(THEME_KEY, a);
  applyAppearance(a);
}

export function setAccent(a: Accent): void {
  accent.set(a);
  if (browser) localStorage.setItem(ACCENT_KEY, a);
  applyAccent(a);
}

/** Wire up initial application + OS change listener. Call once on mount. */
export function initTheme(): () => void {
  if (!browser) return () => {};
  applyAppearance(initialAppearance());
  applyAccent(initialAccent());
  const mq = matchMedia('(prefers-color-scheme: dark)');
  const onChange = () => {
    let cur: Appearance = 'system';
    appearance.subscribe((v) => (cur = v))();
    if (cur === 'system') applyAppearance('system');
  };
  mq.addEventListener('change', onChange);
  return () => mq.removeEventListener('change', onChange);
}
