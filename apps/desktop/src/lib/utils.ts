import { clsx, type ClassValue } from 'clsx';
import { twMerge } from 'tailwind-merge';

/** Merge Tailwind class lists, resolving conflicts. */
export function cn(...inputs: ClassValue[]): string {
  return twMerge(clsx(inputs));
}

/** Human-friendly relative-ish timestamp. */
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

/** Deterministic accent color for a label/tag chip. */
export function chipColor(seed: string): string {
  const palette = [
    'text-violet-300 bg-violet-500/10 border-violet-500/20',
    'text-sky-300 bg-sky-500/10 border-sky-500/20',
    'text-emerald-300 bg-emerald-500/10 border-emerald-500/20',
    'text-amber-300 bg-amber-500/10 border-amber-500/20',
    'text-rose-300 bg-rose-500/10 border-rose-500/20',
    'text-cyan-300 bg-cyan-500/10 border-cyan-500/20'
  ];
  let h = 0;
  for (let i = 0; i < seed.length; i++) h = (h * 31 + seed.charCodeAt(i)) >>> 0;
  return palette[h % palette.length];
}

const PRIORITY_STYLES: Record<string, string> = {
  urgent: 'text-rose-300 bg-rose-500/10 border-rose-500/25',
  high: 'text-orange-300 bg-orange-500/10 border-orange-500/25',
  medium: 'text-amber-300 bg-amber-500/10 border-amber-500/25',
  low: 'text-slate-300 bg-slate-500/10 border-slate-500/25'
};

export function priorityColor(p?: string): string {
  if (!p) return 'text-muted-foreground bg-muted border-border';
  return PRIORITY_STYLES[p.toLowerCase()] ?? 'text-muted-foreground bg-muted border-border';
}
