// Shared crossfade for board cards, so a ticket that moves between lists (from an
// agent's edit picked up by the live poll) flies smoothly to its new place instead
// of teleporting. New/removed cards fall back to a gentle scale.
//
// Animations are SUPPRESSED during a user's own drag: svelte-dnd-action already
// animates that interaction, and layering the crossfade on top looks jittery. The
// board toggles `suppressAnim(true/false)` around a drag.
import { crossfade, scale } from 'svelte/transition';
import { cubicOut } from 'svelte/easing';

let suppressed = false;

/** Suppress (or restore) card move animations - used while the user is dragging. */
export function suppressAnim(v: boolean): void {
  suppressed = v;
}

const DURATION = 260;

export const [send, receive] = crossfade({
  duration: () => (suppressed ? 0 : DURATION),
  easing: cubicOut,
  fallback(node) {
    return scale(node, { duration: suppressed ? 0 : 200, start: 0.92, easing: cubicOut });
  }
});
