<!-- BucketCatalog: the songs a website reads from this bucket (lib/catalog.js). Add one by
     picking its files and filling in what it is; it is uploaded and listed in one go. -->
<script>
  import { ListMusic, LoaderCircle, PenLine, Plus, Trash2, X } from '@lucide/svelte';
  import { scale, fade } from 'svelte/transition';
  import { store } from '../lib/store.svelte.js';
  import { ask } from '../lib/ask.svelte.js';
  import { modalIn, reducedMotion } from '../lib/motion.js';
  import { publishTrack, unpublishTrack } from '../lib/catalog.js';

  /**
   * catalog: the catalog as last read; onsaved(next) is called with it after every save;
   * onclose() closes the panel.
   */
  let { catalog, onsaved = () => {}, onclose = () => {} } = $props();

  const FIELD = 'expo t-hover h-10 w-full border border-hairline bg-white/[0.03] px-3 text-[14px] text-white placeholder:text-text-faint focus:border-chiral/60 focus:outline-none';
  const FILE = 'block w-full min-w-0 text-[12px] text-text-dim file:mr-3 file:h-9 file:cursor-pointer file:border file:border-solid file:border-hairline file:bg-white/[0.04] file:px-3 file:text-[11px] file:font-bold file:uppercase file:tracking-wider file:text-white';
  const LABEL = 'micro mb-1.5 block text-text-faint';
  const FAILED = 'That could not be saved';
  const AUDIO = 'audio/*,.mp3,.m4a,.ogg,.opus,.wav,.flac,.webm';
  const IMAGE = 'image/*';
  const blank = () => ({ id: '', title: '', artist: '', album: '', genre: '', year: new Date().getFullYear(), public: true });

  let form = $state(blank());
  let files = $state({ src: null, instrumental: null, cover: null, altCover: null });
  let busy = $state('');
  let error = $state('');
  let picker = $state(0); // bumped to empty the file inputs

  const editing = $derived(catalog.tracks.find((t) => t.id === form.id) || null);
  // Newest first: the song just added is the one to check.
  const tracks = $derived([...catalog.tracks].reverse());

  function reset() {
    form = blank();
    files = { src: null, instrumental: null, cover: null, altCover: null };
    picker += 1;
    error = '';
  }

  function edit(track) {
    reset();
    form = { ...blank(), ...track };
  }

  const pick = (field) => (e) => (files[field] = e.target.files?.[0] || null);

  /** "https://data.example.com/music/2026/a%20b.mp3" -> "a b.mp3" */
  function fileName(address) {
    const last = address.split('/').pop();
    try {
      return decodeURIComponent(last);
    } catch {
      return last;
    }
  }

  async function save(e) {
    e.preventDefault();
    if (busy) return;
    error = '';
    busy = 'Starting';
    try {
      const title = form.title;
      onsaved(await publishTrack(store.adapter, form, files, (step) => (busy = step)));
      store.toast(editing ? `${title} saved` : `${title} is live`);
      reset();
    } catch (err) {
      error = err?.message || FAILED;
    } finally {
      busy = '';
      // What was uploaded is in the bucket whether or not the catalog was saved.
      store.refresh().catch(() => {});
    }
  }

  async function remove(track) {
    if (busy) return;
    if (!(await ask(`Take “${track.title}” out of the catalog? Its files stay in the bucket.`, { yes: 'Take it out', danger: true }))) return;
    busy = 'Saving the catalog';
    try {
      onsaved(await unpublishTrack(store.adapter, track.id));
      if (form.id === track.id) reset();
      store.toast(`${track.title} taken out`);
    } catch (err) {
      error = err?.message || FAILED;
    } finally {
      busy = '';
    }
  }

  function onKey(e) {
    if (e.key === 'Escape' && !busy) onclose();
  }
</script>

<svelte:window onkeydown={onKey} />

{#snippet fileField(field, label, accept, has)}
  <label class="block min-w-0">
    <span class={LABEL}>{label}</span>
    {#key picker}
      <input type="file" {accept} onchange={pick(field)} class={FILE} />
    {/key}
    {#if has && !files[field]}
      <span class="micro mt-1 block truncate text-text-faint">KEEPS {fileName(has).toUpperCase()}</span>
    {/if}
  </label>
{/snippet}

<div class="fixed inset-0 z-50 grid place-items-center p-3 sm:p-4" role="dialog" aria-modal="true" aria-label="Music catalog">
  <div transition:fade={{ duration: reducedMotion() ? 0 : 200 }} class="absolute inset-0 bg-black/70" onclick={() => { if (!busy) onclose(); }} role="presentation"></div>
  <div transition:scale={modalIn()} class="hud-corners scanlines glass relative z-10 flex max-h-[92dvh] w-full min-w-0 max-w-5xl flex-col 2xl:max-w-6xl" style="transform-origin: center">
    <div class="flex shrink-0 items-center gap-3 border-b border-hairline px-5 py-3.5">
      <ListMusic class="h-5 w-5 text-chiral" strokeWidth={1.75} />
      <div class="min-w-0 flex-1">
        <p class="truncate text-[15px] font-semibold text-white 2xl:text-[17px]">Music catalog</p>
        <p class="micro truncate text-text-faint">{catalog.tracks.length} {catalog.tracks.length === 1 ? 'SONG' : 'SONGS'} · LIVE ON THE SITE THAT READS IT</p>
      </div>
      <button class="expo t-hover p-2 text-text-dim transition-colors hover:text-white disabled:opacity-40" onclick={onclose} disabled={!!busy} aria-label="Close">
        <X class="h-5 w-5" strokeWidth={1.75} />
      </button>
    </div>

    <div class="grid min-h-0 flex-1 grid-cols-1 overflow-y-auto md:grid-cols-[minmax(0,1.15fr)_minmax(0,1fr)] md:overflow-hidden">
      <form class="min-w-0 space-y-4 p-5 md:overflow-y-auto" onsubmit={save}>
        <p class="micro text-chiral">{editing ? `EDITING ${editing.title.toUpperCase()}` : 'ADD A SONG'}</p>
        <div class="grid grid-cols-1 gap-4 sm:grid-cols-2">
          <label class="block min-w-0">
            <span class={LABEL}>TITLE</span>
            <input bind:value={form.title} type="text" required autocomplete="off" class={FIELD} />
          </label>
          <label class="block min-w-0">
            <span class={LABEL}>ARTIST</span>
            <input bind:value={form.artist} type="text" required autocomplete="off" class={FIELD} />
          </label>
          <label class="block min-w-0">
            <span class={LABEL}>ALBUM</span>
            <input bind:value={form.album} type="text" autocomplete="off" class={FIELD} />
          </label>
          <div class="grid grid-cols-2 gap-4">
            <label class="block min-w-0">
              <span class={LABEL}>GENRE</span>
              <input bind:value={form.genre} type="text" autocomplete="off" class={FIELD} />
            </label>
            <label class="block min-w-0">
              <span class={LABEL}>YEAR</span>
              <input bind:value={form.year} type="number" inputmode="numeric" class={FIELD} />
            </label>
          </div>
        </div>
        <div class="grid grid-cols-1 gap-4 sm:grid-cols-2">
          {@render fileField('src', 'SONG (FULL MIX)', AUDIO, editing?.src)}
          {@render fileField('instrumental', 'INSTRUMENTAL', AUDIO, editing?.instrumental)}
          {@render fileField('cover', 'COVER', IMAGE, editing?.cover)}
          {@render fileField('altCover', 'SECOND COVER (PNG OR JPG)', IMAGE, editing?.altCover)}
        </div>
        <label class="flex items-center gap-3">
          <input type="checkbox" bind:checked={form.public} class="h-4 w-4 accent-[var(--color-chiral)]" />
          <span class="text-[13px] text-text-dim">Listed for everyone. Off keeps it for people with the passcode.</span>
        </label>
        {#if error}
          <p class="text-[13px] text-chiral" role="alert">{error}</p>
        {/if}
        <div class="flex gap-2">
          <button type="submit" disabled={!!busy} class="expo t-hover micro flex h-10 min-w-0 flex-1 items-center justify-center gap-2 bg-chiral px-3 font-bold text-black transition-all hover:brightness-110 disabled:opacity-60">
            {#if busy}<LoaderCircle class="h-4 w-4 shrink-0 animate-spin" strokeWidth={2} />{:else if !editing}<Plus class="h-4 w-4 shrink-0" strokeWidth={2.5} />{/if}
            <span class="truncate">{busy ? busy.toUpperCase() : editing ? 'SAVE CHANGES' : 'UPLOAD AND GO LIVE'}</span>
          </button>
          {#if editing}
            <button type="button" disabled={!!busy} class="expo t-hover micro h-10 border border-hairline px-4 font-bold text-white hover:border-white/30 disabled:opacity-60" onclick={reset}>CANCEL</button>
          {/if}
        </div>
      </form>

      <div class="border-t border-hairline md:overflow-y-auto md:border-l md:border-t-0">
        {#if !tracks.length}
          <p class="p-5 text-[13px] text-text-dim">No songs in the catalog yet. The first one you add shows up on the site.</p>
        {/if}
        {#each tracks as track (track.id)}
          <div class="flex items-center gap-3 border-b border-hairline-faint px-5 py-2.5 {form.id === track.id ? 'bg-white/[0.05]' : ''}">
            <div class="min-w-0 flex-1">
              <p class="truncate text-[13px] font-medium text-white">{track.title}</p>
              <p class="micro truncate text-text-faint">{[track.artist, track.album, track.public === false ? 'PASSCODE' : ''].filter(Boolean).join(' · ').toUpperCase()}</p>
            </div>
            <button class="expo t-hover p-2 text-text-dim transition-colors hover:text-white disabled:opacity-40" onclick={() => edit(track)} disabled={!!busy} aria-label={`Edit ${track.title}`}>
              <PenLine class="h-4 w-4" strokeWidth={1.75} />
            </button>
            <button class="expo t-hover p-2 text-text-dim transition-colors hover:text-red-400 disabled:opacity-40" onclick={() => remove(track)} disabled={!!busy} aria-label={`Take ${track.title} out of the catalog`}>
              <Trash2 class="h-4 w-4" strokeWidth={1.75} />
            </button>
          </div>
        {/each}
      </div>
    </div>
  </div>
</div>
