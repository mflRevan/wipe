<script lang="ts">
  import { fade } from 'svelte/transition';
  import { X, Download } from 'lucide-svelte';
  import { lightbox, closeLightbox } from '$lib/stores/lightbox';

  // Images open fitted to the screen; a tap/click toggles 1:1 (scrollable) zoom.
  let zoomed = $state(false);
  let text = $state<string | null>(null);

  $effect(() => {
    const item = $lightbox;
    zoomed = false;
    text = null;
    if (item?.kind === 'text') {
      fetch(item.src)
        .then((r) => r.text())
        .then((t) => {
          if ($lightbox === item) text = t.length > 200_000 ? `${t.slice(0, 200_000)}\n…` : t;
        })
        .catch(() => (text = '(could not load the file)'));
    }
  });

  function onKey(e: KeyboardEvent) {
    if (e.key === 'Escape' && $lightbox) {
      // Handled here: keep an underlying modal from also closing.
      e.preventDefault();
      e.stopPropagation();
      closeLightbox();
    }
  }
</script>

<svelte:window onkeydowncapture={onKey} />

{#if $lightbox}
  {@const item = $lightbox}
  <div
    class="lb"
    transition:fade={{ duration: 120 }}
    role="dialog"
    aria-modal="true"
    aria-label={`Preview of ${item.name}`}
  >
    <button class="scrim" aria-label="Close preview" onclick={closeLightbox}></button>
    <header class="bar">
      <span class="name" title={item.name}>{item.name}</span>
      <a class="ib" href={item.src} download={item.name} aria-label="Download" title="Download"
        ><Download size={16} /></a
      >
      <button class="ib" onclick={closeLightbox} aria-label="Close" title="Close"
        ><X size={18} /></button
      >
    </header>
    <div class="stage" class:zoomed>
      {#if item.kind === 'image'}
        <button class="imgbtn" onclick={() => (zoomed = !zoomed)} aria-label="Toggle zoom">
          <img src={item.src} alt={item.name} />
        </button>
      {:else if item.kind === 'video'}
        <!-- svelte-ignore a11y_media_has_caption -->
        <video src={item.src} controls autoplay playsinline></video>
      {:else if item.kind === 'audio'}
        <audio src={item.src} controls autoplay></audio>
      {:else if item.kind === 'pdf'}
        <iframe src={item.src} title={item.name}></iframe>
      {:else if item.kind === 'text'}
        <pre class="wp-scroll">{text ?? 'Loading…'}</pre>
      {:else}
        <p class="none">No preview for this file type - use Download.</p>
      {/if}
    </div>
  </div>
{/if}

<style>
  .lb {
    position: fixed;
    inset: 0;
    z-index: 200;
    display: flex;
    flex-direction: column;
  }
  .scrim {
    position: absolute;
    inset: 0;
    border: none;
    background: rgba(0, 0, 0, 0.82);
    cursor: zoom-out;
  }
  .bar {
    position: relative;
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 10px 12px;
    padding-top: max(10px, env(safe-area-inset-top));
    color: #fff;
  }
  .name {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: 13px;
  }
  .ib {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 36px;
    height: 36px;
    border-radius: var(--wp-r-sm);
    border: 1px solid rgba(255, 255, 255, 0.2);
    background: rgba(255, 255, 255, 0.08);
    color: #fff;
    cursor: pointer;
  }
  .stage {
    position: relative;
    flex: 1;
    min-height: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 0 12px 16px;
    pointer-events: none;
  }
  .stage > :global(*) {
    pointer-events: auto;
  }
  .stage.zoomed {
    overflow: auto;
    align-items: flex-start;
    justify-content: flex-start;
  }
  .imgbtn {
    border: none;
    padding: 0;
    background: none;
    cursor: zoom-in;
    max-width: 100%;
    max-height: 100%;
    display: flex;
  }
  .imgbtn img {
    max-width: 100%;
    max-height: calc(100dvh - 90px);
    object-fit: contain;
    border-radius: var(--wp-r-sm);
  }
  .zoomed .imgbtn {
    max-width: none;
    max-height: none;
    cursor: zoom-out;
  }
  .zoomed .imgbtn img {
    max-width: none;
    max-height: none;
  }
  video {
    max-width: 100%;
    max-height: calc(100dvh - 90px);
  }
  iframe {
    width: min(900px, 100%);
    height: 100%;
    border: none;
    background: #fff;
    border-radius: var(--wp-r-sm);
  }
  pre {
    width: min(900px, 100%);
    max-height: 100%;
    overflow: auto;
    margin: 0;
    padding: 14px;
    border-radius: var(--wp-r-sm);
    background: var(--wp-card);
    color: var(--wp-text);
    font-family: var(--wp-font-mono);
    font-size: 12px;
    white-space: pre-wrap;
    word-break: break-word;
  }
  .none {
    color: #ddd;
    font-size: 14px;
  }
</style>
