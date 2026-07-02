import adapter from '@sveltejs/adapter-static';
import { vitePreprocess } from '@sveltejs/vite-plugin-svelte';

/** @type {import('@sveltejs/kit').Config} */
const config = {
  preprocess: vitePreprocess(),
  kit: {
    // SPA mode: emit a fallback index.html so client-side routing works when
    // the whole app is served statically (embedded in wipe-daemon or Tauri).
    adapter: adapter({
      fallback: 'index.html',
      pages: 'build',
      assets: 'build'
    })
  }
};

export default config;
