<!-- TheaterPlayer: plays what Theater opens. A film or an episode fills the screen with its
     subtitles; a song plays in a bar along the bottom while you keep browsing; a photo opens
     large, with the arrows (on screen or on the keyboard) walking the album.

     The box gives each item a play link that works without the box token (a video player
     cannot send one), so the browser streams it straight from the box, seeking as it goes.

     Keys while a film plays (a controller in brackets): Space, Enter or K (A) play and pause;
     left and right (D-pad) jump 10 seconds back and on; up and down change the volume; M mutes;
     C (X) turns subtitles on and off; F (Y) fills the screen; Shift+P and Shift+N (LB and RB)
     the previous and next item; Escape (B) closes. Media keys work for films and songs. -->
<script>
  import { X, SkipBack, SkipForward, ChevronLeft, ChevronRight, LoaderCircle, Play, Pause } from '@lucide/svelte';
  import { fade, fly, scale } from 'svelte/transition';
  import { store } from '../lib/store.svelte.js';
  import { prefs } from '../lib/prefs.svelte.js';
  import { episodeCode } from '../lib/theater.js';
  import { modalIn, reducedMotion } from '../lib/motion.js';
  import { PAD, setHints, input } from '../lib/input.svelte.js';

  const SEEK_S = 10;
  const VOLUME_STEP = 0.1;
  const FLASH_MS = 1100;

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

  // ---- the keyboard and a controller ----

  let root = $state(null);
  let video = $state(null);
  let audio = $state(null);
  let audioPlaying = $state(false);
  let flash = $state(''); // a word or two over the picture after a key: "+10 S", "VOLUME 60%"
  let flashTimer = null;

  function show(text) {
    flash = text;
    clearTimeout(flashTimer);
    flashTimer = setTimeout(() => (flash = ''), FLASH_MS);
  }

  const clock = (s) => {
    const t = Math.max(0, Math.floor(s || 0));
    const h = Math.floor(t / 3600);
    const m = Math.floor((t % 3600) / 60);
    const sec = String(t % 60).padStart(2, '0');
    return h ? `${h}:${String(m).padStart(2, '0')}:${sec}` : `${m}:${sec}`;
  };

  function togglePlay(el) {
    if (!el) return;
    if (el.paused) el.play().catch(() => {});
    else el.pause();
    if (el === video) show(el.paused ? 'PAUSED' : 'PLAYING');
  }

  function seek(by) {
    if (!video) return;
    const end = Number.isFinite(video.duration) ? video.duration : Infinity;
    video.currentTime = Math.min(end, Math.max(0, video.currentTime + by));
    const total = Number.isFinite(video.duration) ? ` / ${clock(video.duration)}` : '';
    show(`${by > 0 ? '+' : '-'}${Math.abs(by)} S · ${clock(video.currentTime)}${total}`);
  }

  function changeVolume(by) {
    if (!video) return;
    video.muted = false;
    video.volume = Math.round(Math.min(1, Math.max(0, video.volume + by)) * 10) / 10;
    show(`VOLUME ${Math.round(video.volume * 100)}%`);
  }

  function toggleSubtitles() {
    const tracks = video ? [...video.textTracks] : [];
    if (!tracks.length) return show('NO SUBTITLES');
    const on = tracks.find((t) => t.mode === 'showing');
    for (const t of tracks) t.mode = 'disabled';
    if (on) return show('SUBTITLES OFF');
    tracks[0].mode = 'showing';
    show(`SUBTITLES ${(tracks[0].label || 'ON').toUpperCase()}`);
  }

  function toggleFullscreen() {
    if (document.fullscreenElement) document.exitFullscreen().catch(() => {});
    else root?.requestFullscreen?.().catch(() => show('FULL SCREEN IS NOT AVAILABLE HERE'));
  }

  function onKey(e) {
    if (e.defaultPrevented || !item || e.ctrlKey || e.metaKey || e.altKey) return;
    const k = e.key;
    if (mode === 'audio') {
      // The song plays under Theater, which keeps the keyboard; only media keys are the bar's.
      if (k === 'MediaPlayPause') togglePlay(audio);
      else if (k === 'MediaTrackPrevious') step(-1);
      else if (k === 'MediaTrackNext') step(1);
      else return;
      e.preventDefault();
      return;
    }
    const prev = k === PAD.PREV || k === 'MediaTrackPrevious' || (e.shiftKey && k === 'P') || (mode === 'photo' && k === 'ArrowLeft');
    const next = k === PAD.NEXT || k === 'MediaTrackNext' || (e.shiftKey && k === 'N') || (mode === 'photo' && k === 'ArrowRight');
    if (k === 'Escape') {
      if (document.fullscreenElement) document.exitFullscreen().catch(() => {});
      else onclose();
    } else if (prev) step(-1);
    else if (next) step(1);
    else if (mode === 'video' && video) {
      const onButton = e.target?.closest?.('button');
      if (((k === 'Enter' || k === ' ') && !onButton) || k === 'k' || k === 'MediaPlayPause') togglePlay(video);
      else if (k === 'ArrowLeft' || k === 'j') seek(-SEEK_S);
      else if (k === 'ArrowRight' || k === 'l') seek(SEEK_S);
      else if (k === 'ArrowUp') changeVolume(VOLUME_STEP);
      else if (k === 'ArrowDown') changeVolume(-VOLUME_STEP);
      else if (k === 'm') {
        video.muted = !video.muted;
        show(video.muted ? 'MUTED' : `VOLUME ${Math.round(video.volume * 100)}%`);
      } else if (k === 'c' || k === PAD.X) toggleSubtitles();
      else if (k === 'f' || k === PAD.Y) toggleFullscreen();
      else return;
    } else return;
    e.preventDefault();
  }

  // The controller's buttons for what is showing, along the bottom.
  $effect(() => {
    const many = queue.length > 1;
    if (mode === 'video') {
      return setHints('player', [
        { btn: 'A', label: 'Play / pause' },
        { btn: 'DPAD', label: 'Seek · volume' },
        ...(many ? [{ btn: 'LB/RB', label: 'Previous / next' }] : []),
        { btn: 'X', label: 'Subtitles' },
        { btn: 'Y', label: 'Full screen' },
        { btn: 'B', label: 'Close' },
      ], 3);
    }
    if (mode === 'photo') {
      return setHints('player', [...(many ? [{ btn: 'DPAD', label: 'Photos' }] : []), { btn: 'B', label: 'Close' }], 3);
    }
  });

  // The film or photo arrives after the player opens: focus moves onto it then (off the
  // top bar, where A would press Close).
  $effect(() => {
    if (loading || !root || mode === 'audio') return;
    const frame = requestAnimationFrame(() => root?.querySelector('[data-autofocus]:not(:disabled)')?.focus({ preventScroll: true }));
    return () => cancelAnimationFrame(frame);
  });

  // The music bar sits along the bottom, so the hints move up over it.
  $effect(() => {
    if (mode !== 'audio') return;
    input.lift = true;
    return () => (input.lift = false);
  });
</script>

<!-- on the document, so the player has the keys before the app's own navigation (on the window) -->
<svelte:document onkeydown={onKey} />

{#if mode === 'video' || mode === 'photo'}
  <div bind:this={root} class="fixed inset-0 z-[60] flex flex-col bg-black" role="dialog" aria-modal="true" aria-label={title} transition:fade={{ duration: reducedMotion() ? 0 : 180 }}>
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
          <!-- focused when the player opens, so the keys work at once -->
          <video bind:this={video} tabindex="0" data-autofocus class="h-full max-h-full w-full bg-black outline-none" controls autoplay playsinline crossorigin="anonymous" src={link.url} onended={ended}>
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
          <button data-autofocus class="nav-arrow expo t-hover absolute right-3 top-1/2 -translate-y-1/2 border border-white/15 bg-black/60 p-3 text-white hover:border-white/40 disabled:opacity-20" onclick={() => step(1)} disabled={at >= queue.length - 1} aria-label="Next photo">
            <ChevronRight class="h-6 w-6" />
          </button>
        {/if}
      {/if}
      {#if flash}
        <p class="micro pointer-events-none absolute left-1/2 top-6 -translate-x-1/2 bg-black/75 px-3 py-2 text-white" role="status">{flash}</p>
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
      <!-- the browser's own audio controls cannot be reached with a controller; this can -->
      <button class="expo t-hover grid h-9 w-9 shrink-0 place-items-center border border-hairline text-white hover:border-chiral/60 disabled:opacity-30" onclick={() => togglePlay(audio)} disabled={!link} aria-label={audioPlaying ? 'Pause' : 'Play'}>
        {#if audioPlaying}<Pause class="h-4 w-4" />{:else}<Play class="ml-0.5 h-4 w-4" />{/if}
      </button>
      {#if link}
        {#key link.url}
          <audio bind:this={audio} class="h-9 min-w-0 flex-1" controls autoplay src={link.url} onended={ended} onplay={() => (audioPlaying = true)} onpause={() => (audioPlaying = false)}></audio>
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
