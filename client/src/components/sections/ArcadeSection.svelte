<!-- ArcadeSection: the game library and its save data. The rail has Consoles (a group that
     opens and closes, one row per system) and Saves. A game's card shows its saves; Saves
     lists every save file with every version recorded, and can put an old one back.
     Box art in a grid, one click to open a game's card and play it. On a box, this reads the games folder as it is:
     RetroArch's playlists are the library and its thumbnails are the art.

     A game on several discs is one card with a disc to choose. In a card, the left and right
     arrows (on screen or on the keyboard) move through the games; Shuffle deals random ones,
     from one console or the whole library.

     Steam cache keeps Steam games on the box for every PC in the home (see arcade/SteamCache). -->
<script>
  import { Gamepad2, Search, Play, X, HardDrive, Power, Save, History, RotateCcw, RefreshCw, Dices, ChevronLeft, ChevronRight, Disc3, ExternalLink, HardDriveDownload } from '@lucide/svelte';
  import { untrack } from 'svelte';
  import { scale, fade, fly } from 'svelte/transition';
  import { SvelteMap } from 'svelte/reactivity';
  import { shell } from '../../lib/shell.svelte.js';
  import { store } from '../../lib/store.svelte.js';
  import { fmtSize } from '../../lib/format.js';
  import { prefs } from '../../lib/prefs.svelte.js';
  import { modalIn, itemIn, reducedMotion } from '../../lib/motion.js';
  import SectionFrame from '../SectionFrame.svelte';
  import Notice from '../Notice.svelte';
  import SteamCache from '../arcade/SteamCache.svelte';

  const NO_ARCADE = 'not started with --arcade';
  const CONSOLES = '@consoles'; // rail ids that are not systems
  const SAVES = '@saves';
  const SHUFFLE = '@shuffle';
  const STEAM = '@steam';
  const RECENT_PICKS = 12; // a shuffle does not deal one of the last few games again
  const KIND_LABEL = { battery: 'IN-GAME SAVE', state: 'SAVE STATE' };
  const HASH_SHOWN = 10;
  const ART_MARGIN = '300px'; // start loading art a little before a card scrolls into view
  const STAGGER_CAP = 24; // cards past this many appear without the entry stagger
  // Settings > Arcade > Box art size: how many cards fit across at each screen width.
  const GRID_BY_SIZE = {
    small: 'grid-cols-3 sm:grid-cols-4 lg:grid-cols-6 xl:grid-cols-7 2xl:grid-cols-8',
    medium: 'grid-cols-2 sm:grid-cols-3 lg:grid-cols-4 xl:grid-cols-5 2xl:grid-cols-6',
    large: 'grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4 2xl:grid-cols-4',
  };

  let systems = $state([]);
  let systemId = $state(null);
  let games = $state([]);
  let query = $state('');
  let loading = $state(true);
  let problem = $state(''); // why the library cannot be shown, when it cannot
  let openGame = $state(null);
  let openSystemId = $state(null); // the open card's system: under a library-wide shuffle, not always the one listed
  let disc = $state(0); // which disc of the open game Play starts
  let about = $state(null); // null: not asked or nothing found; 'loading'; or { title, extract, url }
  let deck = $state(null); // null: the card walks the list. { scope, picks, at }: it walks a shuffle
  let cardDir = $state(1); // which way the card last moved, for its slide
  const gamesBySystem = new Map(); // system id -> its games, so a library-wide shuffle reads each list once
  let consolesOpen = $state(true);
  let mode = $state('games'); // 'games' | 'saves'
  let saves = $state([]);
  let savesProblem = $state('');
  let recording = $state(false);
  let openSave = $state(null); // the save whose versions are shown
  let versions = $state([]);
  let restoring = $state('');
  let snap = $state(null); // in-game screenshot for the open card
  const art = new SvelteMap(); // `${system}:${game}` -> image URL, or null once known to be missing

  const source = $derived(shell.arcadeSource);
  const system = $derived(systems.find((s) => s.id === systemId) || null);
  const cardSystem = $derived(systems.find((s) => s.id === openSystemId) || null);
  const shown = $derived(
    query.trim() ? games.filter((g) => g.title.toLowerCase().includes(query.trim().toLowerCase())) : games
  );
  const totalGames = $derived(systems.reduce((sum, s) => sum + s.games, 0));
  const totalBytes = $derived(systems.reduce((sum, s) => sum + s.bytes, 0));
  const railItems = $derived([
    { id: CONSOLES, label: 'Consoles', icon: Gamepad2, count: systems.length, expanded: consolesOpen },
    ...(consolesOpen ? systems.map((s) => ({ id: s.id, label: s.short, count: s.games, indent: true })) : []),
    { id: SAVES, label: 'Saves', icon: Save, count: saves.length },
    { id: SHUFFLE, label: 'Shuffle', icon: Dices },
    { id: STEAM, label: 'Steam cache', icon: HardDriveDownload },
  ]);
  const gameSaves = $derived(openGame ? saves.filter((s) => s.game && s.game.system === openSystemId && s.game.id === openGame.id) : []);
  const unrecorded = $derived(saves.filter((s) => !s.recorded).length);

  const when = (ms) => new Date(ms).toLocaleString(undefined, { dateStyle: 'medium', timeStyle: 'short' });

  function pickRail(id) {
    if (id === CONSOLES) {
      consolesOpen = !consolesOpen;
    } else if (id === SAVES) {
      showSaves();
    } else if (id === SHUFFLE) {
      shuffle('library');
    } else if (id === STEAM) {
      mode = 'steam';
      openGame = null;
    } else {
      mode = 'games';
      pickSystem(id);
    }
  }

  async function loadSaves(from = source) {
    try {
      saves = await from.arcadeSaves();
      savesProblem = '';
    } catch (err) {
      saves = [];
      savesProblem = err?.message || 'Saves could not be read';
    }
  }

  /** Records whatever changed, then lists. Recording only reads the games drive. */
  async function recordSaves() {
    recording = true;
    try {
      const result = await source.arcadeSavesRecord();
      if (result.added) store.toast(`${result.added} new ${result.added === 1 ? 'version' : 'versions'} recorded`);
      await loadSaves();
      if (openSave) versions = await source.arcadeSaveHistory(openSave.id);
    } catch (err) {
      store.toast(err?.message || 'Saves could not be recorded');
    } finally {
      recording = false;
    }
  }

  async function showSaves(save = null) {
    mode = 'saves';
    openGame = null;
    openSave = null;
    versions = [];
    await recordSaves();
    if (save) await pickSave(saves.find((s) => s.id === save.id) || save);
  }

  async function pickSave(save) {
    openSave = save;
    try {
      versions = await source.arcadeSaveHistory(save.id);
    } catch (err) {
      versions = [];
      store.toast(err?.message || 'That save\'s history could not be read');
    }
  }

  async function putBack(version) {
    const question = `Put this version of ${openSave.name} back? It replaces the save file on the games drive. The save as it is now is recorded first, so you can undo this.`;
    if (!window.confirm(question)) return;
    restoring = version.hash;
    try {
      await source.arcadeSaveRestore(openSave.id, version.hash);
      store.toast(source.isDemo ? 'Demo: nothing was written' : 'That version is back in place');
      await loadSaves();
      await pickSave(saves.find((s) => s.id === openSave.id) || openSave);
    } catch (err) {
      store.toast(err?.message || 'That version could not be put back');
    } finally {
      restoring = '';
    }
  }

  /** "Sky Runner (USA) (En,Fr)" -> { name: "Sky Runner", tags: "USA · En,Fr" } */
  function splitTitle(title) {
    const cut = title.indexOf(' (');
    if (cut === -1) return { name: title, tags: '' };
    const tags = [...title.slice(cut).matchAll(/\(([^)]*)\)/g)].map((m) => m[1]).join(' · ');
    return { name: title.slice(0, cut), tags };
  }

  let turningOn = $state(false);

  /** Arcade is off on this box: ask the box to look for a games folder and turn it on. */
  async function turnOn() {
    turningOn = true;
    try {
      await source.arcadeEnable();
      await loadSystems(source);
      store.toast('Arcade is on');
    } catch (err) {
      store.toast(err?.message || 'Arcade could not be turned on');
    } finally {
      turningOn = false;
    }
  }

  async function loadSystems(from) {
    loading = true;
    problem = '';
    systems = [];
    games = [];
    art.clear();
    gamesBySystem.clear();
    if (!from) {
      loading = false;
      return;
    }
    try {
      systems = await from.arcadeSystems();
      if (systems.length) await pickSystem(systems[0].id, from);
    } catch (err) {
      problem = err?.message || 'The library could not be read';
    }
    loading = false;
  }

  /** One system's games, read once per visit. */
  async function gamesOf(id, from = source) {
    if (!gamesBySystem.has(id)) gamesBySystem.set(id, await from.arcadeGames(id));
    return gamesBySystem.get(id);
  }

  async function pickSystem(id, from = source) {
    systemId = id;
    query = '';
    try {
      games = await gamesOf(id, from);
    } catch (err) {
      games = [];
      store.toast(err?.message || 'Could not list that system');
    }
  }

  /** Loads a card's box art the first time it comes near the screen. */
  function lazyArt(node, game) {
    const key = `${systemId}:${game.id}`;
    if (!game.hasBoxart || art.has(key)) return {};
    const from = source;
    const forSystem = systemId;
    const watcher = new IntersectionObserver(
      (entries) => {
        if (!entries.some((e) => e.isIntersecting)) return;
        watcher.disconnect();
        from.arcadeArt(forSystem, game.id, 'boxart').then((url) => art.set(key, url));
      },
      { rootMargin: ART_MARGIN }
    );
    watcher.observe(node);
    return { destroy: () => watcher.disconnect() };
  }

  /** Shows a game's card. Its picture, screenshot and description arrive as they load. */
  async function show(game, ofSystem) {
    openGame = game;
    openSystemId = ofSystem;
    disc = Math.max(0, (game.discs || []).findIndex((d) => d.present));
    snap = null;
    about = null;
    const from = source;
    const still = () => openGame === game && openSystemId === ofSystem && source === from;
    const key = `${ofSystem}:${game.id}`;
    if (game.hasBoxart && !art.has(key)) from.arcadeArt(ofSystem, game.id, 'boxart').then((url) => art.set(key, url));
    if (game.hasSnap) from.arcadeArt(ofSystem, game.id, 'snap').then((url) => { if (still()) snap = url; });
    if (prefs.get('arcade.descriptions') && from.arcadeAbout) {
      about = 'loading';
      try {
        const found = await from.arcadeAbout(ofSystem, game.id);
        if (still()) about = found;
      } catch {
        if (still()) about = null; // no connection, or Wikipedia did not answer: the card just has no description
      }
    }
  }

  /** A click on a game in the grid: its card, with the arrows walking the list as shown. */
  function open(game) {
    deck = null;
    cardDir = 1;
    show(game, systemId);
  }

  function closeCard() {
    openGame = null;
    deck = null;
  }

  /** A random game that is on the drive, from this console or from any, not one dealt lately. */
  async function deal(scope) {
    const recent = deck ? deck.picks.slice(-RECENT_PICKS).map((p) => `${p.systemId}:${p.game.id}`) : [];
    for (let attempt = 0; attempt < 30; attempt++) {
      let ofSystem = systemId;
      if (scope === 'library') {
        // A console with more games comes up more often, so every game is as likely as any other.
        let roll = Math.random() * totalGames;
        ofSystem = (systems.find((s) => (roll -= s.games) < 0) || systems[0]).id;
      }
      const pool = (await gamesOf(ofSystem)).filter((g) => g.present);
      if (!pool.length) continue;
      const game = pool[Math.floor(Math.random() * pool.length)];
      if (!recent.includes(`${ofSystem}:${game.id}`) || attempt > 20) return { systemId: ofSystem, game };
    }
    return null;
  }

  /** Starts a shuffle (or re-deals one with another scope) and shows its first game. */
  async function shuffle(scope) {
    if (!systems.length) return;
    mode = 'games';
    deck = { scope, picks: [], at: -1 };
    await step(1);
  }

  /** The arrows: through the shuffle when one is running, otherwise through the list on screen. */
  async function step(dir) {
    cardDir = dir;
    if (deck) {
      if (dir < 0) {
        if (deck.at <= 0) return;
        deck.at -= 1;
      } else if (deck.at < deck.picks.length - 1) {
        deck.at += 1;
      } else {
        const pick = await deal(deck.scope);
        if (!pick || !deck) return;
        deck.picks.push(pick);
        deck.at = deck.picks.length - 1;
      }
      const pick = deck.picks[deck.at];
      show(pick.game, pick.systemId);
      return;
    }
    if (!openGame || !shown.length) return;
    const at = shown.findIndex((g) => g.id === openGame.id);
    show(shown[(at + dir + shown.length) % shown.length], systemId);
  }

  function onCardKey(e) {
    if (!openGame) return;
    if (e.key === 'Escape') closeCard();
    else if (e.key === 'ArrowRight') step(1);
    else if (e.key === 'ArrowLeft') step(-1);
    else return;
    e.preventDefault();
  }

  async function play(game) {
    try {
      const res = await source.arcadePlay(openSystemId, game.id, disc);
      store.toast(res.demo ? 'Demo: nothing is launched here' : `Starting in ${cardSystem.emulator}`);
      closeCard();
    } catch (err) {
      store.toast(err?.message || 'Could not start the game');
    }
  }

  $effect(() => {
    const from = source; // the only thing this effect should re-run for
    untrack(() => {
      mode = 'games';
      openSave = null;
      saves = [];
      loadSystems(from).then(() => from && loadSaves(from));
    });
  });
</script>

<svelte:window onkeydown={onCardKey} />

<SectionFrame
  title="Arcade"
  subtitle={systems.length ? `${totalGames} games · ${systems.length} systems · ${fmtSize(totalBytes)}` : 'Your game library'}
  tag={store.isDemo ? 'SAMPLE DATA' : ''}
  items={railItems}
  active={mode === 'saves' ? SAVES : mode === 'steam' ? STEAM : systemId}
  onselect={pickRail}
>
  {#snippet actions()}
    {#if mode === 'saves'}
      <button
        disabled={recording}
        onclick={recordSaves}
        class="expo t-hover micro flex h-10 items-center gap-2 border border-hairline px-3 font-bold text-white transition-colors hover:border-chiral/60 disabled:opacity-50"
      >
        <RefreshCw class="h-3.5 w-3.5 {recording ? 'animate-spin' : ''}" /> <span class="hidden sm:inline">{recording ? 'RECORDING' : 'RECORD NOW'}</span>
      </button>
    {:else if games.length && mode !== 'steam'}
      <button
        onclick={() => shuffle('system')}
        class="expo t-hover micro flex h-10 items-center gap-2 border border-hairline px-3 font-bold text-white transition-colors hover:border-chiral/60"
        title="A random game from this console"
      >
        <Dices class="h-4 w-4" strokeWidth={1.75} /> <span class="hidden sm:inline">SHUFFLE</span>
      </button>
      <div class="group relative hidden w-56 sm:block">
        <Search class="pointer-events-none absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-text-faint group-focus-within:text-chiral" strokeWidth={1.75} />
        <input
          bind:value={query}
          type="text"
          placeholder="FIND A GAME"
          aria-label="Find a game in this system"
          class="micro h-10 w-full border border-hairline bg-white/[0.03] pl-9 pr-3 text-white placeholder:text-text-faint focus:border-chiral/60 focus:outline-none"
        />
      </div>
    {/if}
  {/snippet}

  {#if !source}
    <Notice icon={Gamepad2} title="NO BOX CONNECTED">
      Arcade shows the games folder on a Castle box. Connect to one from Storage, or open the
      <a class="text-bridge underline" href="/demo">demo</a> to see it with sample games.
    </Notice>
  {:else if mode === 'steam'}
    <!-- the Steam cache works whether or not this box has a games folder -->
    <SteamCache {source} />
  {:else if loading}
    <p class="micro text-text-faint">READING THE LIBRARY</p>
  {:else if problem.includes(NO_ARCADE)}
    <Notice icon={HardDrive} title="NO GAMES FOLDER ON THIS BOX">
      This box has no games folder yet. Plug in the drive that has <span class="text-white">Games</span> (with
      <span class="text-white">Consoles</span> and <span class="text-white">Roms</span> inside), then turn Arcade on here. No restart needed.
      <button
        disabled={turningOn}
        onclick={turnOn}
        class="expo t-hover hud-corners micro mx-auto mt-4 flex h-11 items-center gap-2 bg-chiral px-5 font-bold text-black transition-all hover:brightness-110 active:scale-[0.98] disabled:opacity-50"
      >
        <Power class="h-4 w-4" strokeWidth={2} /> {turningOn ? 'TURNING ON' : 'FIND MY GAMES'}
      </button>
    </Notice>
  {:else if problem}
    <Notice icon={Gamepad2} title="LIBRARY UNAVAILABLE">{problem}</Notice>
  {:else if !systems.length}
    <Notice icon={Gamepad2} title="NO GAMES FOUND">The games folder has no RetroArch playlists or game files yet.</Notice>
  {:else if mode === 'saves'}
    <div class="mb-4 flex flex-wrap items-baseline gap-x-3 gap-y-1">
      <h2 class="text-[15px] font-semibold text-white">Save data</h2>
      <p class="micro text-text-faint">
        {saves.length} {saves.length === 1 ? 'SAVE' : 'SAVES'} · {saves.reduce((sum, s) => sum + s.versions, 0)} VERSIONS RECORDED
        {#if unrecorded}· <span class="text-chiral">{unrecorded} NOT RECORDED YET</span>{/if}
      </p>
    </div>
    {#if savesProblem}
      <p class="text-[13px] text-chiral">{savesProblem}</p>
    {/if}
    <div class="grid gap-3 xl:grid-cols-[minmax(0,1fr)_24rem]">
      <div class="glass overflow-x-auto p-4">
        <table class="w-full text-left text-[13px]">
          <thead class="micro text-text-faint">
            <tr><th class="pb-2 pr-3 font-normal">GAME</th><th class="pb-2 pr-3 font-normal">KIND</th><th class="pb-2 pr-3 font-normal">CORE</th><th class="pb-2 pr-3 font-normal">LAST CHANGED</th><th class="pb-2 pr-3 text-right font-normal">SIZE</th><th class="pb-2 text-right font-normal">VERSIONS</th></tr>
          </thead>
          <tbody>
            {#each saves as save (save.id)}
              <tr
                class="expo t-hover cursor-pointer border-t border-hairline-faint align-top transition-colors hover:bg-white/[0.04] {openSave?.id === save.id ? 'bg-white/[0.05]' : ''}"
                onclick={() => pickSave(save)}
              >
                <td class="max-w-[20rem] py-2 pr-3">
                  <button class="block w-full truncate text-left {save.game ? 'text-white' : 'text-text-dim'}" title={save.name}>
                    {save.game ? splitTitle(save.game.title).name : save.name}
                  </button>
                  {#if !save.game}<span class="micro text-text-faint">NOT MATCHED TO A GAME</span>{/if}
                </td>
                <td class="micro whitespace-nowrap py-2 pr-3 {save.kind === 'state' ? 'text-bridge' : 'text-chiral'}">{KIND_LABEL[save.kind]}</td>
                <td class="py-2 pr-3 text-text-dim">{save.core}</td>
                <td class="whitespace-nowrap py-2 pr-3 text-text-dim">{when(save.modified)}</td>
                <td class="py-2 pr-3 text-right tabular-nums text-text-dim">{fmtSize(save.size)}</td>
                <td class="py-2 text-right tabular-nums {save.recorded ? 'text-white' : 'text-chiral'}">{save.versions}{save.recorded ? '' : ' +1'}</td>
              </tr>
            {:else}
              <tr><td colspan="6" class="py-6 text-center text-text-dim">No save files found on the games drive yet. Play something and save.</td></tr>
            {/each}
          </tbody>
        </table>
      </div>

      <div class="glass h-fit p-4">
        {#if openSave}
          <p class="micro flex items-center gap-2 text-text-faint"><History class="h-3.5 w-3.5" /> VERSIONS</p>
          <h3 class="mt-1 break-words text-[15px] font-bold text-white">{openSave.game ? splitTitle(openSave.game.title).name : openSave.name}</h3>
          <p class="micro mt-0.5 text-text-faint">{KIND_LABEL[openSave.kind]} · {openSave.core}</p>
          <ol class="mt-3">
            {#each versions as version, i (version.seq)}
              <li class="flex items-center gap-3 border-t border-hairline-faint py-2.5">
                <div class="min-w-0 flex-1">
                  <p class="text-[13px] text-white">{when(version.modified)}</p>
                  <p class="micro mt-0.5 truncate text-text-faint" title={version.hash}>{fmtSize(version.size)} · {version.hash.slice(0, HASH_SHOWN)}</p>
                </div>
                {#if i === 0 && openSave.recorded}
                  <span class="micro text-live">CURRENT</span>
                {:else}
                  <button
                    disabled={!!restoring}
                    onclick={() => putBack(version)}
                    class="expo t-hover micro flex h-8 items-center gap-1.5 border border-hairline px-2.5 font-bold text-white transition-colors hover:border-chiral/60 disabled:opacity-50"
                  >
                    <RotateCcw class="h-3.5 w-3.5 {restoring === version.hash ? 'animate-spin' : ''}" /> PUT BACK
                  </button>
                {/if}
              </li>
            {:else}
              <li class="border-t border-hairline-faint py-3 text-[13px] text-text-dim">Not recorded yet. Press Record now.</li>
            {/each}
          </ol>
        {:else}
          <p class="micro flex items-center gap-2 text-text-faint"><History class="h-3.5 w-3.5" /> VERSIONS</p>
          <p class="mt-2 text-[13px] leading-relaxed text-text-dim">Pick a save to see every version Castle has recorded and put an older one back.</p>
        {/if}
      </div>
    </div>
    <p class="mt-4 max-w-3xl text-[12px] leading-relaxed text-text-dim">
      Each core keeps its own saves, so the same game can appear once per core. Castle copies every version into your vault and never
      changes one it has recorded. Recording only reads the games drive. Put back is the one action that writes to it: it replaces that
      single save file, after recording what was there.
    </p>
  {:else}
    <div class="mb-4 flex flex-wrap items-baseline gap-x-3 gap-y-1">
      <h2 class="text-[15px] font-semibold text-white">{system?.name}</h2>
      <p class="micro text-text-faint">
        {shown.length} {shown.length === 1 ? 'GAME' : 'GAMES'} · {system?.emulator} · {system?.withArt} WITH BOX ART
      </p>
    </div>

    {#key systemId}
      <ul class="grid gap-3 {GRID_BY_SIZE[prefs.get('arcade.cardSize')] || GRID_BY_SIZE.medium}">
        {#each shown as game, i (game.id)}
          {@const title = splitTitle(game.title)}
          {@const cover = art.get(`${systemId}:${game.id}`)}
          <li in:fly={i < STAGGER_CAP ? itemIn(i) : { duration: 0 }}>
            <button
              use:lazyArt={game}
              class="expo t-hover group block w-full border border-hairline bg-white/[0.02] text-left transition-all hover:-translate-y-0.5 hover:border-chiral/60 {game.present ? '' : 'opacity-50'}"
              onclick={() => open(game)}
            >
              <div class="relative grid aspect-[3/4] place-items-center overflow-hidden border-b border-hairline-faint bg-black/50">
                {#if cover}
                  <img src={cover} alt="" draggable="false" class="expo t-ui h-full w-full object-contain transition-transform group-hover:scale-[1.04]" />
                {:else}
                  <div class="flex flex-col items-center gap-2 px-3 text-center">
                    <Gamepad2 class="h-8 w-8 text-text-faint" strokeWidth={1.25} />
                    <span class="micro text-text-faint">{system?.short}</span>
                  </div>
                {/if}
                {#if game.discs?.length > 1}
                  <span class="micro absolute left-1.5 top-1.5 flex items-center gap-1 bg-black/75 px-1.5 py-0.5 text-bridge"><Disc3 class="h-3 w-3" /> {game.discs.length} DISCS</span>
                {/if}
              </div>
              <div class="px-2.5 py-2">
                <p class="truncate text-[13px] font-medium text-white">{title.name}</p>
                <p class="micro mt-0.5 truncate text-text-faint">{(prefs.get('arcade.regionTags') && title.tags) || fmtSize(game.size)}</p>
              </div>
            </button>
          </li>
        {/each}
      </ul>
    {/key}

    {#if !shown.length}
      <p class="micro mt-8 text-center text-text-faint">NO GAME MATCHES "{query}"</p>
    {/if}
  {/if}
</SectionFrame>

{#if openGame}
  {@const title = splitTitle(openGame.title)}
  {@const cover = art.get(`${openSystemId}:${openGame.id}`)}
  {@const discs = openGame.discs || []}
  <div class="fixed inset-0 z-50 grid place-items-center p-4" role="dialog" aria-modal="true" aria-label={title.name}>
    <div
      transition:fade={{ duration: reducedMotion() ? 0 : 200 }}
      class="absolute inset-0 bg-black/75"
      onclick={closeCard}
      onkeydown={(e) => { if (e.key === 'Enter' || e.key === ' ') closeCard(); }}
      role="button"
      tabindex="-1"
      aria-label="Close"
    ></div>

    <!-- the arrows sit at the edges of the screen, clear of the card -->
    <button
      class="nav-arrow left-1 sm:left-4"
      onclick={() => step(-1)}
      disabled={deck ? deck.at <= 0 : shown.length < 2}
      aria-label={deck ? 'The game dealt before this one' : 'Previous game'}
      title="Left arrow"
    >
      <ChevronLeft class="h-7 w-7" strokeWidth={1.5} />
    </button>
    <button
      class="nav-arrow right-1 sm:right-4"
      onclick={() => step(1)}
      disabled={!deck && shown.length < 2}
      aria-label={deck ? 'Deal another game' : 'Next game'}
      title="Right arrow"
    >
      {#if deck && deck.at >= deck.picks.length - 1}
        <Dices class="h-7 w-7" strokeWidth={1.5} />
      {:else}
        <ChevronRight class="h-7 w-7" strokeWidth={1.5} />
      {/if}
    </button>

    <div transition:scale={modalIn()} class="hud-corners scanlines glass relative z-10 flex max-h-[90dvh] w-full max-w-3xl flex-col">
      {#if deck}
        <div class="flex flex-wrap items-center gap-2 border-b border-hairline px-5 py-2.5">
          <p class="micro flex items-center gap-2 text-chiral"><Dices class="h-3.5 w-3.5" /> SHUFFLE</p>
          <div class="ml-auto flex items-center border border-hairline" role="group" aria-label="Shuffle from">
            {#each [{ id: 'system', label: system ? `ONLY ${system.short}` : 'THIS CONSOLE' }, { id: 'library', label: 'WHOLE LIBRARY' }] as choice (choice.id)}
              <button
                class="expo t-hover micro px-2.5 py-1.5 transition-colors {deck.scope === choice.id ? 'bg-white/[0.07] text-chiral' : 'text-text-faint hover:text-white'}"
                onclick={() => shuffle(choice.id)}
                aria-pressed={deck.scope === choice.id}
              >
                {choice.label}
              </button>
            {/each}
          </div>
        </div>
      {/if}

      {#key `${openSystemId}:${openGame.id}`}
        <div in:fly={reducedMotion() ? { duration: 0 } : { x: 36 * cardDir, duration: 220, opacity: 0 }} class="flex min-h-0 flex-1 flex-col sm:flex-row">
          <div class="grid shrink-0 place-items-center bg-black/60 p-5 sm:w-64">
            {#if cover}
              <img src={cover} alt="" class="max-h-56 max-w-full object-contain sm:max-h-80" draggable="false" />
            {:else}
              <Gamepad2 class="h-14 w-14 text-text-faint" strokeWidth={1} />
            {/if}
          </div>
          <div class="flex min-w-0 flex-1 flex-col gap-3 overflow-y-auto p-5">
            <div class="flex items-start gap-3">
              <div class="min-w-0 flex-1">
                <p class="micro text-chiral">{cardSystem?.short}{discs.length > 1 ? ` · ${discs.length} DISCS` : ''}</p>
                <h2 class="mt-1 text-[20px] font-bold leading-tight text-white">{title.name}</h2>
                {#if title.tags}<p class="micro mt-1 text-text-faint">{title.tags}</p>{/if}
              </div>
              <button class="expo t-hover p-1 text-text-dim transition-colors hover:text-white" onclick={closeCard} aria-label="Close">
                <X class="h-5 w-5" strokeWidth={1.75} />
              </button>
            </div>
            {#if snap}
              <img src={snap} alt="In-game screenshot" class="max-h-56 w-full border border-hairline bg-black/40 object-contain" draggable="false" />
            {/if}
            {#if about === 'loading'}
              <p class="micro text-text-faint">LOOKING THIS GAME UP</p>
            {:else if about}
              <div>
                <p class="about text-[13px] leading-relaxed text-text-dim">{about.extract}</p>
                <a class="micro mt-1.5 inline-flex items-center gap-1 text-bridge hover:underline" href={about.url} target="_blank" rel="noreferrer">
                  FROM WIKIPEDIA: {about.title.toUpperCase()} <ExternalLink class="h-3 w-3" />
                </a>
              </div>
            {/if}
            <dl class="micro grid grid-cols-2 gap-x-4 gap-y-2 text-text-faint">
              <div><dt>PLAYS IN</dt><dd class="mt-0.5 text-white">{cardSystem?.emulator}</dd></div>
              <div><dt>SIZE</dt><dd class="mt-0.5 text-white">{fmtSize(openGame.size)}</dd></div>
            </dl>
            {#if discs.length > 1}
              <div>
                <p class="micro mb-1.5 flex items-center gap-2 text-text-faint"><Disc3 class="h-3.5 w-3.5" /> DISC TO PLAY</p>
                <div class="flex flex-wrap gap-1.5" role="radiogroup" aria-label="Disc to play">
                  {#each discs as one, i (i)}
                    <button
                      role="radio"
                      aria-checked={disc === i}
                      disabled={!one.present}
                      onclick={() => (disc = i)}
                      title={one.present ? fmtSize(one.size) : 'This disc is not on the drive'}
                      class="expo t-hover micro border px-3 py-2 font-bold transition-colors disabled:opacity-40
                        {disc === i ? 'border-chiral bg-chiral-dim/40 text-chiral' : 'border-hairline text-white hover:border-white/30'}"
                    >
                      {one.label.toUpperCase()}
                    </button>
                  {/each}
                </div>
              </div>
            {/if}
            <div>
              <p class="micro mb-1 flex items-center gap-2 text-text-faint"><Save class="h-3.5 w-3.5" /> SAVE DATA</p>
              {#each gameSaves as save (save.id)}
                <button
                  class="expo t-hover flex w-full items-center gap-3 border-t border-hairline-faint py-2 text-left transition-colors hover:bg-white/[0.04]"
                  onclick={() => { if (openSystemId !== systemId) pickSystem(openSystemId); showSaves(save); }}
                >
                  <span class="min-w-0 flex-1">
                    <span class="micro block {save.kind === 'state' ? 'text-bridge' : 'text-chiral'}">{KIND_LABEL[save.kind]} · {save.core}</span>
                    <span class="block truncate text-[13px] text-text-dim">{when(save.modified)} · {fmtSize(save.size)}</span>
                  </span>
                  <span class="micro shrink-0 text-text-faint">{save.versions} {save.versions === 1 ? 'VERSION' : 'VERSIONS'}</span>
                </button>
              {:else}
                <p class="border-t border-hairline-faint pt-2 text-[13px] text-text-dim">No save for this game yet.</p>
              {/each}
            </div>
            <div class="mt-auto pt-2">
              {#if !openGame.present}
                <p class="text-[13px] text-text-dim">This game is in the playlist but its file is not on the drive.</p>
              {:else if cardSystem?.canPlay}
                <button
                  class="expo t-hover hud-corners flex h-11 w-full items-center justify-center gap-2 bg-chiral font-bold text-black transition-all hover:brightness-110 active:scale-[0.98]"
                  onclick={() => play(openGame)}
                >
                  <Play class="h-4 w-4 fill-black" strokeWidth={2} />
                  <span class="micro font-bold">PLAY{discs.length > 1 ? ` ${discs[disc]?.label.toUpperCase()}` : ''}</span>
                </button>
                <p class="mt-2 text-[12px] text-text-dim">Opens on the machine the box runs on. Streaming to this screen comes later.</p>
              {:else}
                <p class="text-[13px] text-text-dim">{cardSystem?.emulator} is not on this drive, or the box is not on this machine, so this game cannot be started from here.</p>
              {/if}
            </div>
          </div>
        </div>
      {/key}
      <p class="micro border-t border-hairline-faint px-5 py-2 text-text-faint">
        {#if deck}
          GAME {deck.at + 1} OF THIS SHUFFLE · <span class="text-text-dim">RIGHT</span> DEALS ANOTHER · <span class="text-text-dim">LEFT</span> GOES BACK
        {:else}
          {shown.findIndex((g) => g.id === openGame.id) + 1} OF {shown.length} · <span class="text-text-dim">LEFT</span> AND <span class="text-text-dim">RIGHT</span> MOVE THROUGH THE GAMES
        {/if}
      </p>
    </div>
  </div>
{/if}

<style>
  .nav-arrow {
    position: absolute;
    top: 50%;
    z-index: 20;
    display: grid;
    place-items: center;
    width: 44px;
    height: 72px;
    transform: translateY(-50%);
    color: #fff;
    background: var(--color-panel-solid);
    border: 1px solid var(--color-hairline);
    transition: border-color 150ms var(--ease-expo), color 150ms var(--ease-expo), transform 150ms var(--ease-expo);
  }
  .nav-arrow:hover:not(:disabled) { border-color: var(--color-chiral); color: var(--color-chiral); }
  .nav-arrow:active:not(:disabled) { transform: translateY(-50%) scale(0.94); }
  .nav-arrow:disabled { opacity: 0.3; }
  /* A long article opening is cut to a few lines; the link below it has the rest. */
  .about {
    display: -webkit-box;
    -webkit-box-orient: vertical;
    -webkit-line-clamp: 6;
    line-clamp: 6;
    overflow: hidden;
  }
</style>
