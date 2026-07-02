<script lang="ts">
  import Dialog from './ui/Dialog.svelte';
  import Input from './ui/Input.svelte';
  import Textarea from './ui/Textarea.svelte';
  import Button from './ui/Button.svelte';
  import { api } from '$lib/api';
  import { currentProject, loadBoard } from '$lib/stores/board';
  import { get } from 'svelte/store';

  interface Props {
    open?: boolean;
    listId?: string;
    listName?: string;
  }

  let { open = $bindable(false), listId = '', listName = '' }: Props = $props();

  let title = $state('');
  let type = $state('');
  let priority = $state('medium');
  let body = $state('');
  let submitting = $state(false);
  let error = $state<string | null>(null);

  function reset() {
    title = '';
    type = '';
    priority = 'medium';
    body = '';
    error = null;
  }

  async function submit() {
    if (!title.trim() || submitting) return;
    submitting = true;
    error = null;
    try {
      await api.createTicket(
        {
          title: title.trim(),
          type: type.trim() || undefined,
          priority: priority || undefined,
          body: body.trim() || undefined,
          list: listId || undefined
        },
        get(currentProject) ?? undefined
      );
      await loadBoard();
      reset();
      open = false;
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    } finally {
      submitting = false;
    }
  }
</script>

<Dialog
  bind:open
  title="New ticket"
  description={listName ? `Adding to “${listName}”` : ''}
  onclose={reset}
>
  <form
    class="space-y-4"
    onsubmit={(e) => {
      e.preventDefault();
      void submit();
    }}
  >
    <div class="space-y-1.5">
      <label for="nt-title" class="text-xs font-medium text-muted-foreground">Title</label>
      <!-- svelte-ignore a11y_autofocus -->
      <Input id="nt-title" bind:value={title} placeholder="Short summary" autofocus />
    </div>

    <div class="grid grid-cols-2 gap-3">
      <div class="space-y-1.5">
        <label for="nt-type" class="text-xs font-medium text-muted-foreground">Type</label>
        <Input id="nt-type" bind:value={type} placeholder="feature, bug…" />
      </div>
      <div class="space-y-1.5">
        <label for="nt-priority" class="text-xs font-medium text-muted-foreground">Priority</label>
        <select
          id="nt-priority"
          bind:value={priority}
          class="flex h-9 w-full rounded-md border border-input bg-background px-3 text-sm shadow-sm focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
        >
          <option value="low">low</option>
          <option value="medium">medium</option>
          <option value="high">high</option>
          <option value="urgent">urgent</option>
        </select>
      </div>
    </div>

    <div class="space-y-1.5">
      <label for="nt-body" class="text-xs font-medium text-muted-foreground">Description</label>
      <Textarea id="nt-body" bind:value={body} placeholder="Details (Markdown allowed)…" />
    </div>

    {#if error}
      <p class="text-sm text-destructive">{error}</p>
    {/if}

    <div class="flex justify-end gap-2 pt-2">
      <Button type="button" variant="ghost" onclick={() => (open = false)}>Cancel</Button>
      <Button type="submit" disabled={!title.trim() || submitting}>
        {submitting ? 'Creating…' : 'Create ticket'}
      </Button>
    </div>
  </form>
</Dialog>
