<!-- TheaterSection: films, shows, music, photos and home videos from folders on the box.
     The rail has Recently added, one row per kind of library, and Libraries (the folders,
     who each is for, and adding one). Films are posters; episodes gather into their show and
     songs into their album; photos and videos are pictures. A card opens its details:
     summary, cast, genres and poster from open sources, what the file is, tags, and Play.

     Who sees what follows each library's audience and each item's adults-only mark, by the
     person signed in (see lib/theater.js).

     With a controller: the D-pad walks the posters, A opens one (a show or album opens its
     page, a film its details) and A on Play plays it, B goes back a step, LB and RB change
     library. On a show's page LB and RB change season; in a card they move through the titles. -->
<script>
  import { Clapperboard, Search, Play, X, RefreshCw, FolderPlus, Library, Tag, Shield, Languages, ExternalLink, ChevronLeft, ChevronRight, ChevronUp, Folder, Trash2, LoaderCircle, PenLine, History, Info } from '@lucide/svelte';
  import { untrack } from 'svelte';
  import { scale, fade, fly } from 'svelte/transition';
  import { ask } from '../../lib/ask.svelte.js';
  import { PAD, setHints, focusSoon } from '../../lib/input.svelte.js';
  import { SvelteMap } from 'svelte/reactivity';
  import { shell } from '../../lib/shell.svelte.js';
  import { store } from '../../lib/store.svelte.js';
  import { prefs } from '../../lib/prefs.svelte.js';
  import { fmtSize } from '../../lib/format.js';
  import { fmtStamp } from '../../lib/computer.js';
  import { KINDS, kindById, AUDIENCES, canSeeLibrary, canSeeItem, matches, cardsFor, sortCards, SORTS, fmtLength, episodeCode, resolution } from '../../lib/theater.js';
  import { modalIn, itemIn, reducedMotion } from '../../lib/motion.js';
  import SectionFrame from '../SectionFrame.svelte';
  import Notice from '../Notice.svelte';
  import TheaterPlayer from '../TheaterPlayer.svelte';

  const RECENT = '@recent';
  const LIBRARIES = '@libraries';
  const RECENT_PER_ROW = 12;
  const ART_MARGIN = '400px';
  const STAGGER_CAP = 24;
  const SCAN_POLL_MS = 2500;
  const SHAPES = {
    poster: { box: 'aspect-[2/3]', grid: 'grid-cols-2 sm:grid-cols-3 lg:grid-cols-5 xl:grid-cols-6 2xl:grid-cols-7' },
    square: { box: 'aspect-square', grid: 'grid-cols-2 sm:grid-cols-3 lg:grid-cols-5 xl:grid-cols-6 2xl:grid-cols-7' },
    wide: { box: 'aspect-video', grid: 'grid-cols-2 sm:grid-cols-3 lg:grid-cols-4 xl:grid-cols-5 2xl:grid-cols-6' },
  };

  let overview = $state(null); // { libraries, scanning, ffmpeg, suggestions }
  let items = $state([]);
  let problem = $state('');
  let loading = $state(true);
  let view = $state(RECENT);
  let query = $state('');
  let sort = $state('title');
  let page = $state(null); // an open show or album: a card from cardsFor
  let season = $state(0);
  let openItem = $state(null);
  let answer = $state(null); // details of the open item: { details, episode, probe, poster } or 'loading'
  let cardDir = $state(1);
  let queue = $state([]); // what the player plays
  let playAt = $state(0);
  let fetchingAll = $state(null); // { done, total } while details are fetched for a whole view
  let tagDraft = $state('');
  let fixing = $state(null); // { title, year } while the title used for look-ups is being fixed
  let findingSubs = $state(false);
  const art = new SvelteMap(); // card or item key -> picture address, or null when there is none

  // Adding a library
  let draft = $state({ path: '', kind: 'movies', audience: 'everyone', name: '' });
  let picker = $state(null); // { path, folders } while browsing for a folder
  let adding = $state(false);

  const source = $derived(shell.theaterSource);
  const role = $derived(shell.user.role);
  const canManage = $derived(role === 'Admin');
  const libraries = $derived(overview ? overview.libraries.filter((l) => canSeeLibrary(role, l)) : []);
  const kindOf = $derived(new Map((overview?.libraries || []).map((l) => [l.id, l.kind])));
  const visible = $derived(overview ? items.filter((i) => canSeeItem(role, i, overview.libraries)) : []);
  const byKind = $derived(Object.fromEntries(KINDS.map((k) => [k.id, visible.filter((i) => kindOf.get(i.library) === k.id)])));
  const kindsShown = $derived(KINDS.filter((k) => libraries.some((l) => l.kind === k.id)));
  const cards = $derived(view.startsWith('@') ? [] : sortCards(cardsFor(view, byKind[view].filter((i) => matches(i, query))), sort));
  const railItems = $derived([
    { id: RECENT, label: 'Recently added', icon: History },
    ...kindsShown.map((k) => ({ id: k.id, label: k.label, icon: k.icon, count: cardsFor(k.id, byKind[k.id]).length })),
    ...(canManage ? [{ id: LIBRARIES, label: 'Libraries', icon: Library, count: overview?.libraries.length ?? 0 }] : []),
  ]);
  const recentRows = $derived(
    kindsShown
      .map((k) => ({ kind: k, cards: sortCards(cardsFor(k.id, byKind[k.id].filter((i) => matches(i, query))), 'added').slice(0, RECENT_PER_ROW) }))
      .filter((row) => row.cards.length)
  );
  const pageSeasons = $derived(page && page.lead.kind === 'episode' ? [...new Set(page.items.map((i) => i.season))].sort((a, b) => a - b) : []);
  const pageItems = $derived(page ? (pageSeasons.length > 1 ? page.items.filter((i) => i.season === season) : page.items) : []);
  const subtitle = $derived(
    overview
      ? `${visible.length} ${visible.length === 1 ? 'item' : 'items'} in ${libraries.length} ${libraries.length === 1 ? 'library' : 'libraries'}${overview.scanning ? ' · scanning…' : ''}`
      : ''
  );

  async function load(from = source) {
    loading = true;
    problem = '';
    if (!from) {
      overview = null;
      items = [];
      loading = false;
      return;
    }
    try {
      const [next, list] = await Promise.all([from.theater(), from.theaterItems()]);
      if (from !== source) return;
      overview = next;
      items = list;
      if (view !== LIBRARIES && view !== RECENT && !next.libraries.some((l) => l.kind === view)) view = RECENT;
    } catch (err) {
      problem = err?.message || 'Theater could not be read';
    }
    loading = false;
  }

  $effect(() => {
    const from = source;
    untrack(() => {
      art.clear();
      page = null;
      openItem = null;
      queue = [];
      load(from);
    });
  });

  // While the box is walking its libraries, check back until it is done, then reload.
  $effect(() => {
    if (!overview?.scanning) return;
    const from = source;
    const timer = setInterval(async () => {
      try {
        const next = await from.theater();
        if (!next.scanning) {
          clearInterval(timer);
          await load(from);
        } else {
          overview = next;
        }
      } catch {
        clearInterval(timer);
      }
    }, SCAN_POLL_MS);
    return () => clearInterval(timer);
  });

  function pickRail(id) {
    view = id;
    page = null;
    query = '';
  }

  // ---- pictures ----

  /** The picture a card shows: the poster from the internet for films, shows and albums, else a frame of the file. */
  async function pictureFor(item, wantPoster) {
    const from = source;
    const first = wantPoster ? await from.theaterArt(item.id, 'poster') : null;
    return first || (await from.theaterArt(item.id, 'thumb'));
  }

  const wantsPoster = (item) => item.kind === 'movie' || item.kind === 'episode' || item.kind === 'track';

  function lazyArt(node, card) {
    if (art.has(card.key)) return {};
    const watcher = new IntersectionObserver(
      (entries) => {
        if (!entries.some((e) => e.isIntersecting)) return;
        watcher.disconnect();
        pictureFor(card.lead, wantsPoster(card.lead)).then((url) => art.set(card.key, url));
      },
      { rootMargin: ART_MARGIN }
    );
    watcher.observe(node);
    return { destroy: () => watcher.disconnect() };
  }

  function lazyThumb(node, item) {
    const key = `thumb:${item.id}`;
    if (art.has(key)) return {};
    const watcher = new IntersectionObserver(
      (entries) => {
        if (!entries.some((e) => e.isIntersecting)) return;
        watcher.disconnect();
        source.theaterArt(item.id, 'thumb').then((url) => art.set(key, url));
      },
      { rootMargin: ART_MARGIN }
    );
    watcher.observe(node);
    return { destroy: () => watcher.disconnect() };
  }

  /** After details bring a new poster, every card of that title shows it. */
  async function refreshPoster(item) {
    const url = await source.theaterArt(item.id, 'poster', String(Date.now()));
    if (!url) return;
    const owner = cardsFor(kindOf.get(item.library), byKind[kindOf.get(item.library)]).find((c) => c.items.some((i) => i.id === item.id));
    if (owner) art.set(owner.key, url);
    art.set(`poster:${item.id}`, url);
  }

  // ---- cards and details ----

  /** Back from a show's or an album's page, with focus on its poster again. */
  function closePage() {
    const key = page?.key;
    page = null;
    if (key) focusSoon(`[data-nav-key="${CSS.escape(key)}"]`);
  }

  function openCard(card) {
    const kind = kindOf.get(card.lead.library);
    if ((kind === 'shows' || kind === 'music') && card.items.length > 0) {
      page = card;
      pageAnswer = null;
      season = card.items[0].season;
      cardDir = 1;
      if (prefs.get('theater.lookups')) loadDetails(card.lead, { quiet: true, intoPage: true });
      return;
    }
    if (card.lead.kind === 'photo') {
      play(cards.map((c) => c.lead), cards.indexOf(card));
      return;
    }
    show(card.lead);
  }

  let pageAnswer = $state(null);

  async function loadDetails(item, { quiet = false, intoPage = false } = {}) {
    const from = source;
    const fetch = prefs.get('theater.lookups');
    try {
      const found = await from.theaterDetails(item.id, fetch);
      if (from !== source) return;
      if (intoPage) {
        if (page?.lead.id === item.id) pageAnswer = found;
      } else if (openItem?.id === item.id) {
        answer = found;
      }
      if (found.poster) refreshPoster(item);
    } catch (err) {
      if (!quiet) store.toast(err?.message || 'Details could not be found');
      if (!intoPage && openItem?.id === item.id) answer = { details: null, episode: null, probe: null, poster: false };
    }
  }

  function show(item) {
    openItem = item;
    answer = 'loading';
    tagDraft = '';
    fixing = null;
    if (!art.has(`poster:${item.id}`)) pictureFor(item, wantsPoster(item)).then((url) => art.set(`poster:${item.id}`, url));
    loadDetails(item);
  }

  /** The list the details card's arrows walk: the page's items, or the cards on screen. */
  const walk = $derived(page ? pageItems : cards.map((c) => c.lead));

  function step(dir) {
    if (!openItem || !walk.length) return;
    cardDir = dir;
    const at = walk.findIndex((i) => i.id === openItem.id);
    show(walk[(at + dir + walk.length) % walk.length]);
  }

  function closeItem() {
    openItem = null;
    answer = null;
  }

  function onKey(e) {
    if (e.defaultPrevented) return;
    if (queue.length && queue[playAt]?.kind !== 'track') return; // the player has the keyboard
    if (e.target?.closest?.('input, textarea, select')) return;
    if (openItem) {
      // With a controller, left and right in the row of buttons (Play, subtitles, fix title)
      // move between them, and LB / RB move through the titles. On a keyboard the arrows
      // always move through the titles, and Tab reaches the buttons.
      const inRow = e.fromPad && e.target?.closest?.('[data-nav-row]');
      if (e.key === 'Escape') closeItem();
      else if ((e.key === 'ArrowRight' && !inRow) || e.key === PAD.NEXT) step(1);
      else if ((e.key === 'ArrowLeft' && !inRow) || e.key === PAD.PREV) step(-1);
      else return;
      e.preventDefault();
    } else if (page && e.key === 'Escape') {
      closePage();
      e.preventDefault();
    } else if (page && pageSeasons.length > 1 && (e.key === PAD.NEXT || e.key === PAD.PREV)) {
      // On a show's page, LB and RB change season.
      const at = pageSeasons.indexOf(season);
      season = pageSeasons[(at + (e.key === PAD.NEXT ? 1 : -1) + pageSeasons.length) % pageSeasons.length];
      e.preventDefault();
    }
  }

  // What the controller's buttons do here, along the bottom of the screen.
  $effect(() => {
    if (openItem) {
      return setHints('theater', [
        { btn: 'A', label: 'Select' },
        { btn: 'LB/RB', label: 'Titles' },
        { btn: 'B', label: 'Close' },
      ], 2);
    }
    if (page) {
      return setHints('theater', [
        { btn: 'A', label: 'Select' },
        ...(pageSeasons.length > 1 ? [{ btn: 'LB/RB', label: 'Season' }] : []),
        { btn: 'B', label: 'Back' },
      ]);
    }
    return setHints('theater', [
      { btn: 'A', label: 'Open' },
      { btn: 'LB/RB', label: 'Library' },
      { btn: 'LT/RT', label: 'Page' },
      { btn: 'START', label: 'Sections' },
    ]);
  });

  function play(list, at = 0) {
    queue = list;
    playAt = at;
  }

  /** What Play plays for an item: the rest of its show or album after it, or it alone. */
  function playFrom(item) {
    const group = page && page.items.some((i) => i.id === item.id) ? page.items : null;
    if (group && (item.kind === 'episode' || item.kind === 'track')) play(group, group.findIndex((i) => i.id === item.id));
    else play([item]);
  }

  // ---- changing an item ----

  async function saveEdit(changes) {
    try {
      const updated = await source.theaterEdit({ id: openItem.id, ...changes });
      items = items.map((i) => (i.id === updated.id ? updated : i));
      openItem = updated;
      return updated;
    } catch (err) {
      store.toast(err?.message || 'That could not be saved');
      return null;
    }
  }

  function addTag() {
    const tag = tagDraft.trim();
    if (!tag) return;
    tagDraft = '';
    saveEdit({ tags: [...(openItem.tags || []), tag] });
  }

  async function applyFix() {
    const year = Number(fixing.year) || 0;
    const updated = await saveEdit({ title: fixing.title, year });
    fixing = null;
    if (updated) {
      answer = 'loading';
      loadDetails(updated);
    }
  }

  async function findSubtitles() {
    findingSubs = true;
    try {
      const { found } = await source.theaterSubtitles(openItem.id);
      store.toast(found ? 'Subtitles found. They play with the video.' : 'OpenSubtitles had none for this');
    } catch (err) {
      store.toast(err?.message || 'Subtitles could not be found');
    } finally {
      findingSubs = false;
    }
  }

  /** Details for every title in the view, one after another, so the grid fills with posters. */
  async function fetchAll() {
    const seen = new Set();
    const leads = cardsFor(view, byKind[view])
      .map((c) => c.lead)
      .filter((i) => wantsPoster(i) && !seen.has(`${i.show}|${i.album}|${i.title}`) && seen.add(`${i.show}|${i.album}|${i.title}`));
    fetchingAll = { done: 0, total: leads.length };
    for (const item of leads) {
      if (!fetchingAll) break;
      try {
        const found = await source.theaterDetails(item.id, true);
        if (found.poster) await refreshPoster(item);
      } catch {
        /* one title the sources do not know does not stop the rest */
      }
      if (fetchingAll) fetchingAll = { ...fetchingAll, done: fetchingAll.done + 1 };
    }
    fetchingAll = null;
  }

  // ---- libraries ----

  async function scan(id = null) {
    try {
      await source.theaterScan(id);
      overview = { ...overview, scanning: true };
      store.toast('Scanning');
    } catch (err) {
      store.toast(err?.message || 'The scan could not start');
    }
  }

  async function browse(path) {
    try {
      picker = { path, folders: await source.theaterFolders(path) };
    } catch (err) {
      store.toast(err?.message || 'That folder could not be opened');
    }
  }

  const parentOf = (path) => {
    const trimmed = path.replace(/[\\/]+$/, '');
    const cut = Math.max(trimmed.lastIndexOf('\\'), trimmed.lastIndexOf('/'));
    if (cut < 0) return '';
    const up = trimmed.slice(0, cut);
    return /^[A-Za-z]:$/.test(up) ? `${up}\\` : up || '/';
  };

  async function addLibrary(from = draft) {
    if (!from.path.trim()) return store.toast('Choose a folder first');
    adding = true;
    try {
      const made = await source.theaterLibraryAdd({ ...from, path: from.path.trim() });
      store.toast(`${made.name} added. Scanning it now.`);
      draft = { path: '', kind: 'movies', audience: 'everyone', name: '' };
      picker = null;
      await load();
      overview = { ...overview, scanning: true };
    } catch (err) {
      store.toast(err?.message || 'That folder could not be added');
    } finally {
      adding = false;
    }
  }

  async function setAudience(library, audience) {
    try {
      await source.theaterLibraryUpdate({ id: library.id, audience });
      await load();
    } catch (err) {
      store.toast(err?.message || 'That could not be saved');
    }
  }

  async function removeLibrary(library) {
    if (!(await ask(`Take ${library.name} out of Theater? The folder and its files stay where they are.`, { yes: 'Take it out', danger: true }))) return;
    try {
      await source.theaterLibraryRemove(library.id);
      await load();
      store.toast(`${library.name} taken out`);
    } catch (err) {
      store.toast(err?.message || 'That could not be removed');
    }
  }

  const audienceLabel = (id) => AUDIENCES.find((a) => a.id === id)?.label || id;
</script>

<!-- on the document, so Theater has the keys before the app's own navigation (on the window) -->
<svelte:document onkeydown={onKey} />

{#snippet card(c, i, shape)}
  {@const kind = kindById(kindOf.get(c.lead.library))}
  {@const Icon = kind.icon}
  <li in:fly={itemIn(Math.min(i, STAGGER_CAP))}>
    <button class="nav-card expo t-hover group block w-full text-left" data-nav-key={c.key} onclick={() => openCard(c)} use:lazyArt={c}>
      <div class="relative {SHAPES[shape].box} overflow-hidden border border-hairline bg-white/[0.03] transition-all group-hover:border-white/25">
        {#if art.get(c.key)}
          <img src={art.get(c.key)} alt="" class="expo t-ui h-full w-full object-cover transition-transform group-hover:scale-[1.04]" loading="lazy" draggable="false" />
        {:else}
          <div class="grid h-full w-full place-items-center bg-gradient-to-br from-white/[0.04] to-transparent">
            <Icon class="h-8 w-8 text-text-faint" strokeWidth={1.25} />
          </div>
        {/if}
        {#if c.lead.adultsOnly}
          <span class="micro absolute left-1.5 top-1.5 bg-black/70 px-1.5 py-0.5 text-chiral">18+</span>
        {/if}
        {#if c.items.length > 1}
          <span class="micro absolute right-1.5 top-1.5 bg-black/70 px-1.5 py-0.5 text-white">{c.items.length}</span>
        {/if}
        <div class="nav-reveal expo t-hover absolute inset-0 grid place-items-center bg-black/40 opacity-0 transition-opacity group-hover:opacity-100">
          <span class="grid h-11 w-11 place-items-center rounded-full border border-white/40 bg-black/50">
            {#if c.items.length > 1}<Info class="h-5 w-5 text-white" />{:else}<Play class="ml-0.5 h-5 w-5 text-white" />{/if}
          </span>
        </div>
      </div>
      <p class="mt-2 truncate text-[13px] font-medium text-white">{c.title}</p>
      {#if c.subtitle}<p class="micro mt-0.5 truncate text-text-faint">{c.subtitle}</p>{/if}
    </button>
  </li>
{/snippet}

{#snippet actions()}
  <label class="relative hidden w-56 md:block">
    <Search class="pointer-events-none absolute left-2.5 top-1/2 h-3.5 w-3.5 -translate-y-1/2 text-text-faint" />
    <input
      type="search"
      bind:value={query}
      placeholder="SEARCH TITLES, PEOPLE, TAGS"
      class="micro h-9 w-full border border-hairline bg-white/[0.03] pl-8 pr-2 text-white placeholder:text-text-faint focus:border-chiral/60 focus:outline-none"
      aria-label="Search Theater"
    />
  </label>
  {#if !view.startsWith('@')}
    <button class="micro expo t-hover hidden h-9 border border-hairline px-2.5 text-text-dim transition-colors hover:text-white sm:block" onclick={() => (sort = SORTS[(SORTS.findIndex((s) => s.id === sort) + 1) % SORTS.length].id)} title="Sort">
      {SORTS.find((s) => s.id === sort).label}
    </button>
  {/if}
  {#if source && overview && canManage}
    <button class="micro expo t-hover flex h-9 items-center gap-1.5 border border-hairline px-2.5 text-text-dim transition-colors hover:text-white disabled:opacity-50" onclick={() => scan()} disabled={overview.scanning} title="Look for new files in every library">
      <RefreshCw class="h-3.5 w-3.5 {overview.scanning ? 'animate-spin' : ''}" />
      <span class="hidden lg:inline">{overview.scanning ? 'SCANNING' : 'SCAN'}</span>
    </button>
  {/if}
{/snippet}

<SectionFrame title="Theater" {subtitle} tag={source?.isDemo ? 'SAMPLE DATA' : ''} items={railItems} active={view} onselect={pickRail} {actions}>
  {#if !source}
    <Notice icon={Clapperboard} title="NO BOX CONNECTED">Theater plays films, shows, music and photos from folders on your Castle box. Connect to a box to see them, or switch to Demo.</Notice>
  {:else if !overview && !problem}
    <div class="grid min-h-[40dvh] place-items-center"><LoaderCircle class="h-6 w-6 animate-spin text-text-faint" /></div>
  {:else if problem}
    <Notice icon={Clapperboard} title="THEATER IS NOT AVAILABLE">{problem}</Notice>
  {:else if view === LIBRARIES}
    <!-- ---------------- libraries ---------------- -->
    <div class="mx-auto max-w-4xl px-4 py-6 md:px-6">
      <section class="hud-corners glass px-5 py-5" aria-labelledby="add-heading">
        <h2 id="add-heading" class="micro flex items-center gap-2 text-chiral"><FolderPlus class="h-4 w-4" />ADD A FOLDER</h2>
        <p class="mt-1.5 text-[13px] text-text-dim">Theater reads the folder and everything in it. Nothing in the folder is changed.</p>
        {#if overview.suggestions.length}
          <div class="mt-3 flex flex-wrap gap-2">
            {#each overview.suggestions as s (s.path)}
              <button class="micro expo t-hover border border-bridge/40 px-2.5 py-1.5 text-bridge transition-colors hover:bg-bridge/10" onclick={() => addLibrary({ path: s.path, kind: s.kind, audience: 'family', name: s.name })} disabled={adding}>
                + {s.name.toUpperCase()} AS {kindById(s.kind).label.toUpperCase()}
              </button>
            {/each}
          </div>
        {/if}
        <div class="mt-4 grid gap-3 md:grid-cols-[1fr_auto]">
          <div class="flex gap-2">
            <input bind:value={draft.path} placeholder="FOLDER ON THE BOX, e.g. D:\Media\Films" class="micro h-10 min-w-0 flex-1 border border-hairline bg-white/[0.03] px-3 normal-case text-white placeholder:text-text-faint focus:border-chiral/60 focus:outline-none" aria-label="Folder" />
            <button class="micro expo t-hover flex h-10 items-center gap-1.5 border border-hairline px-3 text-text-dim hover:text-white" onclick={() => browse(draft.path ? draft.path : '')}>
              <Folder class="h-3.5 w-3.5" />BROWSE
            </button>
          </div>
          <div class="flex gap-2">
            <select bind:value={draft.kind} class="micro h-10 border border-hairline bg-black px-2 text-white" aria-label="What the folder holds">
              {#each KINDS as k (k.id)}<option value={k.id}>{k.label.toUpperCase()}</option>{/each}
            </select>
            <select bind:value={draft.audience} class="micro h-10 border border-hairline bg-black px-2 text-white" aria-label="Who can see it">
              {#each AUDIENCES as a (a.id)}<option value={a.id}>{a.label.toUpperCase()}</option>{/each}
            </select>
            <button class="micro expo t-hover h-10 bg-chiral px-4 font-bold text-black transition-opacity hover:opacity-90 disabled:opacity-50" onclick={() => addLibrary()} disabled={adding}>
              {adding ? 'ADDING…' : 'ADD'}
            </button>
          </div>
        </div>
        {#if picker}
          <div class="mt-3 border border-hairline bg-black/40" transition:fade={{ duration: 120 }}>
            <div class="flex items-center gap-2 border-b border-hairline-faint px-3 py-2">
              <button class="expo t-hover p-1 text-text-dim hover:text-white disabled:opacity-30" onclick={() => browse(parentOf(picker.path))} disabled={!picker.path} aria-label="Up one folder">
                <ChevronUp class="h-4 w-4" />
              </button>
              <p class="micro min-w-0 flex-1 truncate normal-case text-text-dim">{picker.path || 'DRIVES'}</p>
              {#if picker.path}
                <button class="micro expo t-hover border border-chiral/50 px-2.5 py-1 text-chiral hover:bg-chiral/10" onclick={() => { draft.path = picker.path; picker = null; }}>USE THIS FOLDER</button>
              {/if}
              <button class="expo t-hover p-1 text-text-faint hover:text-white" onclick={() => (picker = null)} aria-label="Close"><X class="h-4 w-4" /></button>
            </div>
            <ul class="max-h-64 overflow-y-auto py-1">
              {#each picker.folders as f (f.path)}
                <li>
                  <button class="expo t-hover flex w-full items-center gap-2 px-3 py-1.5 text-left text-[13px] text-white hover:bg-white/[0.05]" onclick={() => browse(f.path)}>
                    <Folder class="h-4 w-4 text-chiral" strokeWidth={1.75} />{f.name}
                  </button>
                </li>
              {:else}
                <li class="micro px-3 py-2 text-text-faint">NO FOLDERS IN HERE</li>
              {/each}
            </ul>
          </div>
        {/if}
        {#if !overview.ffmpeg}
          <p class="micro mt-3 text-text-faint">FFMPEG IS NOT INSTALLED ON THE BOX: VIDEOS WILL HAVE NO PICTURES OR RUNNING TIMES UNTIL IT IS.</p>
        {/if}
      </section>

      <section class="mt-6" aria-labelledby="libs-heading">
        <h2 id="libs-heading" class="micro mb-3 text-chiral">LIBRARIES</h2>
        {#if !overview.libraries.length}
          <p class="text-[13px] text-text-faint">None yet. Add a folder above.</p>
        {/if}
        <div class="space-y-2">
          {#each overview.libraries as library, i (library.id)}
            {@const kind = kindById(library.kind)}
            {@const Icon = kind.icon}
            <article in:fly={itemIn(i)} class="flex flex-wrap items-center gap-x-4 gap-y-2 border border-hairline bg-white/[0.02] px-4 py-3">
              <Icon class="h-6 w-6 shrink-0 text-chiral" strokeWidth={1.25} />
              <div class="min-w-0 flex-1">
                <p class="truncate text-[14px] font-semibold text-white">{library.name} <span class="micro ml-1 text-text-faint">{kind.label.toUpperCase()}</span></p>
                <p class="micro mt-0.5 truncate normal-case text-text-faint">{library.path}</p>
                <p class="micro mt-0.5 text-text-faint">{library.count} {library.count === 1 ? 'FILE' : 'FILES'} · {library.scanned ? `SCANNED ${fmtStamp(library.scanned).toUpperCase()}` : 'NOT SCANNED YET'}</p>
              </div>
              <label class="flex items-center gap-1.5">
                <Shield class="h-3.5 w-3.5 text-text-faint" />
                <select value={library.audience} onchange={(e) => setAudience(library, e.target.value)} class="micro h-8 border border-hairline bg-black px-1.5 text-white" aria-label={`Who can see ${library.name}`}>
                  {#each AUDIENCES as a (a.id)}<option value={a.id}>{a.label.toUpperCase()}</option>{/each}
                </select>
              </label>
              <button class="micro expo t-hover flex h-8 items-center gap-1 border border-hairline px-2.5 text-text-dim hover:text-white" onclick={() => scan(library.id)} disabled={overview.scanning}>
                <RefreshCw class="h-3 w-3" />SCAN
              </button>
              <button class="expo t-hover p-1.5 text-text-faint hover:text-red-400" onclick={() => removeLibrary(library)} aria-label={`Take ${library.name} out of Theater`}>
                <Trash2 class="h-4 w-4" />
              </button>
            </article>
          {/each}
        </div>
        <p class="micro mt-4 leading-relaxed text-text-faint">
          WHO CAN SEE A LIBRARY: {AUDIENCES.map((a) => `${a.label.toUpperCase()} — ${a.help.toUpperCase()}`).join(' · ')} UNTIL THE BOX HAS SIGN-IN FOR EACH PERSON, THE APP KEEPS TO THESE.
        </p>
      </section>
    </div>
  {:else if !overview.libraries.length}
    <Notice icon={Clapperboard} title="NOTHING IN THEATER YET">
      Add a folder of films, shows, music, photos or home videos.
      {#if canManage}<button class="micro mt-3 block w-full border border-chiral/50 px-3 py-2 text-chiral hover:bg-chiral/10" onclick={() => pickRail(LIBRARIES)}>ADD A FOLDER</button>{/if}
    </Notice>
  {:else if page}
    <!-- ---------------- a show or an album ---------------- -->
    {@const lead = page.lead}
    {@const d = pageAnswer?.details}
    <div class="px-4 py-5 md:px-6" in:fly={{ x: 24 * cardDir, duration: reducedMotion() ? 0 : 220 }}>
      <button class="micro expo t-hover mb-4 flex items-center gap-1 text-text-dim hover:text-white" onclick={closePage} data-nav-back>
        <ChevronLeft class="h-4 w-4" />{kindById(kindOf.get(lead.library)).label.toUpperCase()}
      </button>
      <div class="flex flex-col gap-6 md:flex-row">
        <div class="w-44 shrink-0 md:w-56">
          <div class="{lead.kind === 'track' ? 'aspect-square' : 'aspect-[2/3]'} overflow-hidden border border-hairline bg-white/[0.03]">
            {#if art.get(page.key)}<img src={art.get(page.key)} alt="" class="h-full w-full object-cover" />{/if}
          </div>
        </div>
        <div class="min-w-0 flex-1">
          <h2 class="text-[26px] font-black leading-tight text-white">{page.title}</h2>
          <p class="micro mt-1.5 text-text-faint">
            {[lead.kind === 'track' ? lead.artist : '', d?.year || page.year || '', page.subtitle, ...(d?.genres || []).slice(0, 3)].filter(Boolean).join(' · ').toUpperCase()}
          </p>
          {#if d?.overview}<p class="mt-3 max-w-3xl text-[14px] leading-relaxed text-text-dim">{d.overview}</p>{/if}
          {#if d?.cast?.length}<p class="micro mt-3 text-text-faint">WITH {d.cast.slice(0, 6).join(', ').toUpperCase()}</p>{/if}
          {#if d?.source}
            <p class="micro mt-2 text-text-faint">DETAILS: {d.source.toUpperCase()}{#if d.url} · <a class="text-bridge hover:underline" href={d.url} target="_blank" rel="noreferrer noopener">READ MORE</a>{/if}</p>
          {/if}
          <div class="mt-4 flex gap-2">
            <button data-autofocus class="micro expo t-hover flex items-center gap-1.5 bg-chiral px-4 py-2.5 font-bold text-black hover:opacity-90" onclick={() => play(pageItems, 0)}>
              <Play class="h-4 w-4" />{lead.kind === 'track' ? 'PLAY ALBUM' : pageSeasons.length > 1 ? `PLAY SEASON ${season}` : 'PLAY FROM THE START'}
            </button>
          </div>
        </div>
      </div>

      {#if pageSeasons.length > 1}
        <div class="mt-6 flex flex-wrap gap-1.5" role="tablist" aria-label="Seasons">
          {#each pageSeasons as s (s)}
            <button role="tab" aria-selected={season === s} class="micro expo t-hover border px-3 py-1.5 transition-colors {season === s ? 'border-chiral/70 text-chiral' : 'border-hairline text-text-dim hover:text-white'}" onclick={() => (season = s)}>
              {s ? `SEASON ${s}` : 'EXTRAS'}
            </button>
          {/each}
        </div>
      {/if}

      <ol class="mt-4 overflow-hidden border border-hairline">
        {#each pageItems as it, i (it.id)}
          <li class="group flex items-center gap-3 border-b border-hairline-faint px-3 py-2 last:border-0 hover:bg-white/[0.03]">
            {#if it.kind === 'episode'}
              <div class="aspect-video w-28 shrink-0 overflow-hidden border border-hairline-faint bg-black/40" use:lazyThumb={it}>
                {#if art.get(`thumb:${it.id}`)}<img src={art.get(`thumb:${it.id}`)} alt="" class="h-full w-full object-cover" />{/if}
              </div>
            {:else}
              <span class="micro w-6 shrink-0 text-right text-text-faint">{it.track || i + 1}</span>
            {/if}
            <button class="min-w-0 flex-1 text-left" onclick={() => show(it)}>
              <p class="truncate text-[13px] text-white">{it.kind === 'episode' ? `${episodeCode(it)} · ${it.title}` : it.title}</p>
              {#if it.kind === 'track' && it.artist !== lead.artist}<p class="micro truncate text-text-faint">{it.artist}</p>{/if}
            </button>
            {#if it.subtitles?.length}<Languages class="h-3.5 w-3.5 shrink-0 text-text-faint" aria-label="Has subtitles" />{/if}
            <button class="expo t-hover grid h-8 w-8 shrink-0 place-items-center border border-hairline text-text-dim hover:border-chiral/60 hover:text-chiral" onclick={() => play(page.items, page.items.indexOf(it))} aria-label={`Play ${it.title}`}>
              <Play class="ml-0.5 h-3.5 w-3.5" />
            </button>
          </li>
        {/each}
      </ol>
    </div>
  {:else if view === RECENT}
    <!-- ---------------- recently added ---------------- -->
    <div class="space-y-8 px-4 py-5 md:px-6">
      {#each recentRows as row (row.kind.id)}
        <section aria-labelledby={`recent-${row.kind.id}`}>
          <div class="mb-3 flex items-baseline justify-between">
            <h2 id={`recent-${row.kind.id}`} class="micro text-chiral">NEW IN {row.kind.label.toUpperCase()}</h2>
            <button class="micro expo t-hover text-text-faint hover:text-white" onclick={() => pickRail(row.kind.id)}>SEE ALL</button>
          </div>
          <ul class="grid gap-3 {SHAPES[row.kind.shape].grid}">
            {#each row.cards.slice(0, row.kind.shape === 'wide' ? 6 : 7) as c, i (c.key)}
              {@render card(c, i, row.kind.shape)}
            {/each}
          </ul>
        </section>
      {:else}
        <Notice icon={Search} title={query ? 'NOTHING MATCHES' : 'NOTHING HERE YET'}>{query ? 'Try fewer words.' : overview.scanning ? 'The box is scanning your libraries.' : 'Scan the libraries, or add a folder.'}</Notice>
      {/each}
    </div>
  {:else}
    <!-- ---------------- one kind ---------------- -->
    {@const kind = kindById(view)}
    <div class="px-4 py-5 md:px-6">
      {#if kind.id === 'movies' || kind.id === 'shows' || kind.id === 'music'}
        <div class="mb-4 flex items-center justify-end gap-3">
          {#if fetchingAll}
            <span class="micro text-text-faint">FINDING DETAILS {fetchingAll.done} / {fetchingAll.total}</span>
            <button class="micro expo t-hover text-text-dim hover:text-white" onclick={() => (fetchingAll = null)}>STOP</button>
          {:else}
            <button class="micro expo t-hover flex items-center gap-1.5 border border-hairline px-2.5 py-1.5 text-text-dim hover:text-white" onclick={fetchAll} title="Ask the open sources about every title here, one at a time">
              <Info class="h-3.5 w-3.5" />GET DETAILS FOR ALL
            </button>
          {/if}
        </div>
      {/if}
      {#if cards.length}
        <ul class="grid gap-3 {SHAPES[kind.shape].grid}">
          {#each cards as c, i (c.key)}
            {@render card(c, i, kind.shape)}
          {/each}
        </ul>
      {:else}
        <Notice icon={Search} title={query ? 'NOTHING MATCHES' : 'NOTHING HERE YET'}>{query ? 'Try fewer words.' : 'Scan the library to pick up new files.'}</Notice>
      {/if}
    </div>
  {/if}
</SectionFrame>

<!-- ---------------- an item's details ---------------- -->
{#if openItem}
  {@const it = openItem}
  {@const loaded = answer && answer !== 'loading' ? answer : null}
  {@const d = loaded?.details}
  {@const probe = loaded?.probe}
  <div class="fixed inset-0 z-50 grid place-items-center p-3 md:p-6" role="dialog" aria-modal="true" aria-label={it.title}>
    <div class="absolute inset-0 bg-black/75" transition:fade={{ duration: reducedMotion() ? 0 : 180 }} onclick={closeItem} onkeydown={() => {}} role="presentation"></div>
    <button class="nav-arrow expo t-hover absolute left-2 top-1/2 z-20 -translate-y-1/2 border border-white/15 bg-black/60 p-2.5 text-white hover:border-white/40 md:left-5" onclick={() => step(-1)} tabindex="-1" aria-label="Previous"><ChevronLeft class="h-5 w-5" /></button>
    <button class="nav-arrow expo t-hover absolute right-2 top-1/2 z-20 -translate-y-1/2 border border-white/15 bg-black/60 p-2.5 text-white hover:border-white/40 md:right-5" onclick={() => step(1)} tabindex="-1" aria-label="Next"><ChevronRight class="h-5 w-5" /></button>
    {#key it.id}
      <div in:fly={{ x: 30 * cardDir, duration: reducedMotion() ? 0 : 220 }} class="hud-corners glass relative z-10 flex max-h-[90dvh] w-full max-w-4xl flex-col overflow-hidden md:flex-row">
        <div class="relative w-full shrink-0 bg-black/50 md:w-72">
          <div class="{it.kind === 'movie' || it.kind === 'episode' ? 'aspect-[2/3]' : it.kind === 'track' ? 'aspect-square' : 'aspect-video'} max-h-[40dvh] w-full md:max-h-none">
            {#if art.get(`poster:${it.id}`)}<img src={art.get(`poster:${it.id}`)} alt="" class="h-full w-full object-cover" />{/if}
          </div>
        </div>
        <div class="min-w-0 flex-1 overflow-y-auto px-5 py-5">
          <div class="flex items-start gap-3">
            <div class="min-w-0 flex-1">
              {#if it.kind === 'episode'}<p class="micro text-chiral">{it.show.toUpperCase()} · {episodeCode(it)}</p>{/if}
              {#if it.kind === 'track'}<p class="micro text-chiral">{[it.artist, it.album].filter(Boolean).join(' · ').toUpperCase()}</p>{/if}
              <h2 class="mt-0.5 text-[22px] font-black leading-tight text-white">{loaded?.episode?.name || it.title}</h2>
              <p class="micro mt-1.5 text-text-faint">
                {[it.year || d?.year || '', fmtLength(probe?.seconds) || (d?.runtimeMinutes ? `${d.runtimeMinutes} min` : ''), resolution(probe), ...(d?.genres || []).slice(0, 3)].filter(Boolean).join(' · ').toUpperCase()}
              </p>
            </div>
            <button class="expo t-hover p-1.5 text-text-dim hover:text-white" onclick={closeItem} aria-label="Close"><X class="h-5 w-5" /></button>
          </div>

          <div class="mt-4 flex flex-wrap gap-2" data-nav-row>
            <button data-autofocus class="micro expo t-hover flex items-center gap-1.5 bg-chiral px-4 py-2.5 font-bold text-black hover:opacity-90" onclick={() => playFrom(it)}>
              <Play class="h-4 w-4" />{it.kind === 'photo' ? 'VIEW' : 'PLAY'}
            </button>
            {#if it.kind !== 'track' && it.kind !== 'photo'}
              <button class="micro expo t-hover flex items-center gap-1.5 border border-hairline px-3 py-2.5 text-text-dim hover:text-white disabled:opacity-50" onclick={findSubtitles} disabled={findingSubs} title="Ask OpenSubtitles for English subtitles">
                {#if findingSubs}<LoaderCircle class="h-3.5 w-3.5 animate-spin" />{:else}<Languages class="h-3.5 w-3.5" />{/if}FIND SUBTITLES
              </button>
            {/if}
            {#if canManage && (it.kind === 'movie' || it.kind === 'episode')}
              <button class="micro expo t-hover flex items-center gap-1.5 border border-hairline px-3 py-2.5 text-text-dim hover:text-white" onclick={() => (fixing = { title: it.title, year: it.year || '' })} title="The title and year used to look this up">
                <PenLine class="h-3.5 w-3.5" />FIX TITLE
              </button>
            {/if}
          </div>

          {#if fixing}
            <div class="mt-3 flex flex-wrap gap-2" transition:fade={{ duration: 120 }}>
              <input bind:value={fixing.title} class="h-9 min-w-0 flex-1 border border-hairline bg-white/[0.03] px-2 text-[13px] text-white focus:border-chiral/60 focus:outline-none" aria-label="Title" />
              <input bind:value={fixing.year} inputmode="numeric" class="h-9 w-20 border border-hairline bg-white/[0.03] px-2 text-[13px] text-white focus:border-chiral/60 focus:outline-none" aria-label="Year" placeholder="YEAR" />
              <button class="micro border border-chiral/60 px-3 text-chiral hover:bg-chiral/10" onclick={applyFix}>LOOK UP AGAIN</button>
            </div>
          {/if}

          {#if answer === 'loading'}
            <p class="micro mt-4 flex items-center gap-2 text-text-faint"><LoaderCircle class="h-3.5 w-3.5 animate-spin" />FINDING DETAILS</p>
          {/if}
          {#if loaded?.episode?.overview}
            <p class="mt-4 text-[14px] leading-relaxed text-text-dim">{loaded.episode.overview}</p>
            {#if loaded.episode.aired}<p class="micro mt-1 text-text-faint">FIRST SHOWN {loaded.episode.aired}</p>{/if}
          {/if}
          {#if d?.overview && !(loaded?.episode?.overview)}
            <p class="mt-4 text-[14px] leading-relaxed text-text-dim">{d.overview}</p>
          {/if}
          {#if d?.director}
            <p class="micro mt-3 text-text-faint">{it.kind === 'track' ? 'BY' : 'DIRECTED BY'} <span class="text-white">{d.director.toUpperCase()}</span></p>
          {/if}
          {#if d?.cast?.length}
            <div class="mt-3 flex flex-wrap gap-1.5">
              {#each d.cast as person (person)}<span class="micro border border-hairline px-2 py-1 text-text-dim">{person.toUpperCase()}</span>{/each}
            </div>
          {/if}
          {#if d?.source}
            <p class="micro mt-3 text-text-faint">DETAILS: {d.source.toUpperCase()}{#if d.url} · <a class="inline-flex items-center gap-1 text-bridge hover:underline" href={d.url} target="_blank" rel="noreferrer noopener">READ MORE<ExternalLink class="h-3 w-3" /></a>{/if}</p>
          {:else if loaded && !d && (it.kind === 'movie' || it.kind === 'episode' || (it.kind === 'track' && it.album))}
            <p class="micro mt-4 text-text-faint">{prefs.get('theater.lookups') ? 'THE OPEN SOURCES DID NOT RECOGNISE THIS TITLE. FIX TITLE AND TRY AGAIN.' : 'LOOKING UP DETAILS IS OFF IN SETTINGS > THEATER.'}</p>
          {/if}

          <!-- tags and who can see it -->
          <div class="mt-5 border-t border-hairline-faint pt-4">
            <p class="micro mb-2 flex items-center gap-1.5 text-text-faint"><Tag class="h-3.5 w-3.5" />TAGS</p>
            <div class="flex flex-wrap items-center gap-1.5">
              {#each it.tags || [] as tag (tag)}
                <span class="micro flex items-center gap-1 border border-bridge/40 px-2 py-1 text-bridge">
                  {tag.toUpperCase()}
                  <button class="hover:text-white" onclick={() => saveEdit({ tags: it.tags.filter((t) => t !== tag) })} aria-label={`Remove tag ${tag}`}><X class="h-3 w-3" /></button>
                </span>
              {/each}
              <input
                bind:value={tagDraft}
                onkeydown={(e) => { if (e.key === 'Enter') { e.preventDefault(); addTag(); } }}
                placeholder="ADD A TAG"
                class="micro h-7 w-32 border border-hairline bg-transparent px-2 text-white placeholder:text-text-faint focus:border-bridge/60 focus:outline-none"
                aria-label="Add a tag"
              />
            </div>
            {#if canManage}
              <label class="mt-3 flex cursor-pointer items-center gap-2 text-[13px] text-text-dim">
                <input type="checkbox" checked={it.adultsOnly} onchange={(e) => saveEdit({ adultsOnly: e.target.checked })} class="accent-[var(--color-chiral)]" />
                Adults only: hidden from children and guests
              </label>
            {/if}
          </div>

          <!-- the file -->
          <dl class="micro mt-5 grid grid-cols-[auto_1fr] gap-x-4 gap-y-1.5 border-t border-hairline-faint pt-4 text-text-faint">
            <dt>FILE</dt><dd class="truncate normal-case text-text-dim" title={it.path}>{it.path}</dd>
            <dt>SIZE</dt><dd class="text-text-dim">{fmtSize(it.size)}</dd>
            {#if probe?.video || probe?.audio}<dt>CODECS</dt><dd class="text-text-dim">{[probe.video, probe.audio].filter(Boolean).join(' / ').toUpperCase()}{probe.width ? ` · ${probe.width}×${probe.height}` : ''}</dd>{/if}
            {#if it.subtitles?.length}<dt>SUBTITLES</dt><dd class="text-text-dim">{it.subtitles.map((s) => s.label).join(', ').toUpperCase()}</dd>{/if}
            <dt>ADDED</dt><dd class="text-text-dim">{fmtStamp(it.added).toUpperCase()}</dd>
          </dl>
        </div>
      </div>
    {/key}
  </div>
{/if}

{#if queue.length}
  <TheaterPlayer {queue} bind:at={playAt} {source} onclose={() => (queue = [])} />
{/if}
