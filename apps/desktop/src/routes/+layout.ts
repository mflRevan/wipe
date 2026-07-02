// Full SPA: no server-side rendering, no prerendering. The whole app builds to
// static files (adapter-static fallback) so it can be embedded in a binary.
export const ssr = false;
export const prerender = false;
export const trailingSlash = 'ignore';
