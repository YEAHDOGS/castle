<!-- TheaterPlayer: plays what Theater opens. A film or an episode fills the screen with its
     subtitles; a song plays in a bar along the bottom while you keep browsing; a photo opens
     large, with the arrows (on screen or on the keyboard) walking the album.

     The box gives each item a play link that works without the box token (a video player
     cannot send one), so the browser streams it straight from the box, seeking as it goes. -->
<script>
  import { X, SkipBack, SkipForward, ChevronLeft, ChevronRight, LoaderCircle } from '@lucide/svelte';
  import { fade, fly, scale } from 'svelte/transition';
  import { store } from '../lib/store.svelte.js';
  import { prefs } from '../lib/prefs.svelte.js';
  import { episodeCode } from '../lib/theater.js';
  import { modalIn, reducedMotion } from '../lib/motion.js';

  /**
   * queue: the items in play order. at: which one is playing (bindable).
   * source: where play links come from. onclose: called when the player is shut.
   */
  let { queue = [], at = $bindable(0), source, onclose = () => {} } = $props();

  const item = $derived(queue[at] || null);
  const mode = $derived(!item ? null : item.kind === 'track' ? 'audio' : item.kind === 'photo' ? 'photo' : 'video');
  let link = $state(null); // { url, mime, subtitles } for the item playing
  let fallback = $state(null); // a picture to show when there is no file to open (the demo)
  let loading = $state(false);
  let cover = $state(null);

  $effect(() => {
    const current = item;
    if (!current) return;
    link = null;
    fallback = null;
    loading = true;
    cover = null;
    const from = source;
    if (current.kind === 'track') {
      from.theaterArt(current.id, 'poster').then(async (url) => {
        const art = url || (await from.theaterArt(current.id, 'thumb'));
        if (item === current) cover = art;
      });
    }
    from
      .theaterPlay(current.id)
      .then((next) => {
        if (item === current) link = next;
      })
      .catch(async (err) => {
        if (item !== current) return;
        if (current.kind === 'photo') {
          fallback = await from.theaterArt(current.id, 'thumb');
        } else {
          store.toast(err?.message || 'That could not be played');
          if (current.kind !== 'track') onclose();
        }
      })
      .finally(() => {
        if (item === current) loading = false;
      });
  });

  const title = $derived(
    !item ? '' : item.kind === 'episode' ? `${item.show} · ${episodeCode(item)} · ${item.title}` : item.kind === 'track' ? item.title : item.year ? `${item.title} (${item.year})` : item.title
  );

  function step(dir) {
    const next = at + dir;
    if (next >= 0 && next < queue.length) at = next;
  }

  function ended() {
    if (prefs.get('theater.autoplay') && at < queue.length - 1) at += 1;
  }

  function onKey(e) {
    if (!item || mode === 'audio') return;
    if (e.key === 'Escape') onclose();
    else if (mode === 'photo' && e.key === 'ArrowRight') step(1);
    else if (mode === 'photo' && e.key === 'ArrowLeft') step(-1);
    else return;
    e.preventDefault();
  }
</script>

<svelte:window onkeydown={onKey} />

{#if mode === 'video' || mode === 'photo'}
  <div class="fixed inset-0 z-[60] flex flex-col bg-black" role="dialog" aria-modal="true" aria-label={title} transition:fade={{ duration: reducedMotion() ? 0 : 180 }}>
    <div class="flex shrink-0 items-center gap-3 border-b border-white/10 bg-black/80 px-4 py-2.5">
      <p class="min-w-0 flex-1 truncate text-[14px] font-semibold text-white">{title}</p>
      {#if queue.length > 1}
        <span class="micro text-text-faint">{at + 1} OF {queue.length}</span>
        <button class="expo t-hover p-1.5 text-text-dim hover:text-white disabled:opacity-30" onclick={() => step(-1)} disabled={at === 0} aria-label="Previous">
          <SkipBack class="h-4 w-4" />
        </button>
        <button class="expo t-hover p-1.5 text-text-dim hover:text-white disabled:opacity-30" onclick={() => step(1)} disabled={at >= queue.length - 1} aria-label="Next">
          <SkipForward class="h-4 w-4" />
        </button>
      {/if}
      <button class="expo t-hover p-1.5 text-text-dim hover:text-white" onclick={onclose} aria-label="Close player">
        <X class="h-5 w-5" />
      </button>
    </div>
    <div class="relative grid min-h-0 flex-1 place-items-center">
      {#if loading}
        <LoaderCircle class="h-8 w-8 animate-spin text-text-faint" />
      {:else if mode === 'video' && link}
        {#key link.url}
          <!-- crossorigin lets the subtitles load from the box when the app is served elsewhere -->
          <video class="h-full max-h-full w-full bg-black" controls autoplay playsinline crossorigin="anonymous" src={link.url} onended={ended}>
            {#each link.subtitles as sub, i (sub.url)}
              <track kind="subtitles" label={sub.label} srclang={sub.label.length === 2 ? sub.label : 'en'} src={sub.url} default={i === 0} />
            {/each}
          </video>
        {/key}
      {:else if mode === 'photo' && (link || fallback)}
        {#key item.id}
          <img in:scale={modalIn()} src={link ? link.url : fallback} alt={item.title} class="max-h-full max-w-full object-contain" draggable="false" />
        {/key}
        {#if queue.length > 1}
          <button class="nav-arrow expo t-hover absolute left-3 top-1/2 -translate-y-1/2 border border-white/15 bg-black/60 p-3 text-white hover:border-white/40 disabled:opacity-20" onclick={() => step(-1)} disabled={at === 0} aria-label="Previous photo">
            <ChevronLeft class="h-6 w-6" />
          </button>
          <button class="nav-arrow expo t-hover absolute right-3 top-1/2 -translate-y-1/2 border border-white/15 bg-black/60 p-3 text-white hover:border-white/40 disabled:opacity-20" onclick={() => step(1)} disabled={at >= queue.length - 1} aria-label="Next photo">
            <ChevronRight class="h-6 w-6" />
          </button>
        {/if}
      {/if}
    </div>
  </div>
{:else if mode === 'audio'}
  <div class="glass fixed inset-x-0 bottom-0 z-[55] border-x-0 border-b-0" role="region" aria-label="Now playing" transition:fly={{ y: 40, duration: reducedMotion() ? 0 : 220 }}>
    <div class="mx-auto flex max-w-5xl items-center gap-3 px-4 py-2.5">
      <div class="h-12 w-12 shrink-0 overflow-hidden border border-hairline bg-black/40">
        {#if cover}<img src={cover} alt="" class="h-full w-full object-cover" />{/if}
      </div>
      <div class="min-w-0 w-48 shrink-0">
        <p class="truncate text-[13px] font-semibold text-white">{item.title}</p>
        <p class="micro truncate text-text-faint">{[item.artist, item.album].filter(Boolean).join(' · ').toUpperCase()}</p>
      </div>
      <button class="expo t-hover p-1.5 text-text-dim hover:text-white disabled:opacity-30" onclick={() => step(-1)} disabled={at === 0} aria-label="Previous song">
        <SkipBack class="h-4 w-4" />
      </button>
      {#if link}
        {#key link.url}
          <audio class="h-9 min-w-0 flex-1" controls autoplay src={link.url} onended={ended}></audio>
        {/key}
      {:else}
        <div class="flex flex-1 items-center gap-2 text-text-faint">
          {#if loading}<LoaderCircle class="h-4 w-4 animate-spin" />{/if}
          <span class="micro">{loading ? 'OPENING…' : 'NOTHING TO PLAY'}</span>
        </div>
      {/if}
      <button class="expo t-hover p-1.5 text-text-dim hover:text-white disabled:opacity-30" onclick={() => step(1)} disabled={at >= queue.length - 1} aria-label="Next song">
        <SkipForward class="h-4 w-4" />
      </button>
      <button class="expo t-hover p-1.5 text-text-dim hover:text-white" onclick={onclose} aria-label="Stop and close">
        <X class="h-4 w-4" />
      </button>
    </div>
  </div>
{/if}
