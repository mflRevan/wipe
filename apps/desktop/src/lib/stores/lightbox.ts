// One app-wide media viewer: any component can open an image/video/PDF/text
// preview full-screen (tap to zoom, Escape or tap outside to close).
import { writable } from 'svelte/store';
import type { MediaKind } from '$lib/utils';

export type LightboxItem = {
  /** URL to show: a media URL for saved attachments, an object URL for staged files. */
  src: string;
  name: string;
  kind: MediaKind;
};

export const lightbox = writable<LightboxItem | null>(null);

export function openLightbox(item: LightboxItem) {
  lightbox.set(item);
}

export function closeLightbox() {
  lightbox.set(null);
}
