<script lang="ts">
  import '@fontsource-variable/space-grotesk';
  import '@fontsource-variable/geist';
  import '@fontsource-variable/geist-mono';
  import '../app.css';
  import { onMount } from 'svelte';
  import type { Snippet } from 'svelte';
  import { initTheme } from '$lib/stores/theme';

  let { children }: { children: Snippet } = $props();

  onMount(() => initTheme());
</script>

{@render children()}
