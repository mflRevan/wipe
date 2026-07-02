<script lang="ts">
  import { projects, currentProject, health } from '$lib/stores/board';
  import { ChevronsUpDown, Check, FolderGit2 } from 'lucide-svelte';

  interface Props {
    onselect?: (path: string) => void;
  }

  let { onselect }: Props = $props();

  let open = $state(false);

  let currentName = $derived.by(() => {
    const p = $projects.find((x) => x.path === $currentProject);
    return p?.name ?? ($currentProject ? $currentProject : 'No project');
  });

  function pick(path: string) {
    open = false;
    if (path !== $currentProject) onselect?.(path);
  }
</script>

<div class="relative">
  <button
    class="flex h-9 items-center gap-2 rounded-md border border-border bg-card px-3 text-sm font-medium transition-colors hover:bg-accent"
    onclick={() => (open = !open)}
    disabled={!$health}
  >
    <FolderGit2 class="h-4 w-4 text-primary" />
    <span class="max-w-[180px] truncate">{currentName}</span>
    <ChevronsUpDown class="h-3.5 w-3.5 text-muted-foreground" />
  </button>

  {#if open}
    <button class="fixed inset-0 z-10 cursor-default" aria-label="Close menu" onclick={() => (open = false)}
    ></button>
    <div
      class="absolute left-0 top-full z-20 mt-1.5 max-h-80 w-72 overflow-y-auto rounded-lg border border-border bg-card p-1 shadow-xl"
    >
      {#if $projects.length === 0}
        <p class="px-3 py-4 text-center text-xs text-muted-foreground">No projects found</p>
      {/if}
      {#each $projects as p (p.path)}
        <button
          class="flex w-full items-center gap-2 rounded-md px-2.5 py-2 text-left text-sm transition-colors hover:bg-accent"
          onclick={() => pick(p.path)}
        >
          <Check
            class="h-3.5 w-3.5 shrink-0 {p.path === $currentProject
              ? 'text-primary'
              : 'text-transparent'}"
          />
          <span class="min-w-0 flex-1">
            <span class="block truncate font-medium">{p.name}</span>
            <span class="block truncate text-[11px] text-muted-foreground">{p.path}</span>
          </span>
        </button>
      {/each}
    </div>
  {/if}
</div>
