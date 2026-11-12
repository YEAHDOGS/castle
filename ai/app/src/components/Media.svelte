<script>
  /** @type {{ src: string, mime: string, onOpen?: () => void, large?: boolean }} */
  let { src, mime, onOpen, large = false } = $props()

  const kind = $derived(mime.split('/')[0])
</script>

{#if kind === 'image'}
  {#if large}
    <img {src} alt="Generated" class="max-w-full max-h-full object-contain" />
  {:else}
    <button class="block w-full aspect-square bg-black/40" onclick={onOpen} aria-label="Open image">
      <img {src} alt="Generated" loading="lazy" class="w-full h-full object-cover" />
    </button>
  {/if}
{:else if kind === 'video'}
  <!-- svelte-ignore a11y_media_has_caption -->
  <video {src} controls loop autoplay={large} muted={!large} playsinline class={large ? 'max-w-full max-h-full' : 'w-full aspect-video bg-black object-contain'}></video>
{:else}
  <div class="p-4 {large ? 'w-full max-w-xl' : ''}"><audio {src} controls class="w-full"></audio></div>
{/if}
