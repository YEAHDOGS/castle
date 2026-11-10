<!-- ArcadeSection: the game library. Systems in the rail, box art in a grid, one click to
     open a game's card and play it. On a box, this reads the games folder as it is:
     RetroArch's playlists are the library and its thumbnails are the art. -->
<script>
  import { Gamepad2, Search, Play, X, HardDrive, Power } from '@lucide/svelte';
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

  const NO_ARCADE = 'not started with --arcade';
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
  let snap = $state(null); // in-game screenshot for the open card
  const art = new SvelteMap(); // `${system}:${game}` -> image URL, or null once known to be missing

  const source = $derived(shell.arcadeSource);
  const system = $derived(systems.find((s) => s.id === systemId) || null);
  const shown = $derived(
    query.trim() ? games.filter((g) => g.title.toLowerCase().includes(query.trim().toLowerCase())) : games
  );
  const totalGames = $derived(systems.reduce((sum, s) => sum + s.games, 0));
  const totalBytes = $derived(systems.reduce((sum, s) => sum + s.bytes, 0));
  const railItems = $derived(systems.map((s) => ({ id: s.id, label: s.short, count: s.games })));

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

  async function pickSystem(id, from = source) {
    systemId = id;
    query = '';
    try {
      games = await from.arcadeGames(id);
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

  async function open(game) {
    openGame = game;
    snap = null;
    if (game.hasSnap && !source.isDemo) snap = await source.arcadeArt(systemId, game.id, 'snap');
  }

  async function play(game) {
    try {
      const res = await source.arcadePlay(systemId, game.id);
      store.toast(res.demo ? 'Demo: nothing is launched here' : `Starting in ${system.emulator}`);
      openGame = null;
    } catch (err) {
      store.toast(err?.message || 'Could not start the game');
    }
  }

  $effect(() => {
    const from = source; // the only thing this effect should re-run for
    untrack(() => loadSystems(from));
  });
</script>

<svelte:window onkeydown={(e) => { if (e.key === 'Escape' && openGame) openGame = null; }} />

<SectionFrame
  title="Arcade"
  subtitle={systems.length ? `${totalGames} games · ${systems.length} systems · ${fmtSize(totalBytes)}` : 'Your game library'}
  tag={store.isDemo ? 'SAMPLE DATA' : ''}
  items={railItems}
  active={systemId}
  onselect={(id) => pickSystem(id)}
>
  {#snippet actions()}
    {#if games.length}
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
  {@const cover = art.get(`${systemId}:${openGame.id}`)}
  <div class="fixed inset-0 z-50 grid place-items-center p-4" role="dialog" aria-modal="true" aria-label={title.name}>
    <div
      transition:fade={{ duration: reducedMotion() ? 0 : 200 }}
      class="absolute inset-0 bg-black/75"
      onclick={() => (openGame = null)}
      onkeydown={(e) => { if (e.key === 'Enter' || e.key === ' ') openGame = null; }}
      role="button"
      tabindex="-1"
      aria-label="Close"
    ></div>
    <div transition:scale={modalIn()} class="hud-corners scanlines glass relative z-10 flex max-h-[88dvh] w-full max-w-2xl flex-col sm:flex-row">
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
            <p class="micro text-chiral">{system?.short}</p>
            <h2 class="mt-1 text-[20px] font-bold leading-tight text-white">{title.name}</h2>
            {#if title.tags}<p class="micro mt-1 text-text-faint">{title.tags}</p>{/if}
          </div>
          <button class="expo t-hover p-1 text-text-dim transition-colors hover:text-white" onclick={() => (openGame = null)} aria-label="Close">
            <X class="h-5 w-5" strokeWidth={1.75} />
          </button>
        </div>
        {#if snap}
          <img src={snap} alt="In-game screenshot" class="max-h-44 w-full border border-hairline object-contain" draggable="false" />
        {/if}
        <dl class="micro grid grid-cols-2 gap-x-4 gap-y-2 text-text-faint">
          <div><dt>PLAYS IN</dt><dd class="mt-0.5 text-white">{system?.emulator}</dd></div>
          <div><dt>SIZE</dt><dd class="mt-0.5 text-white">{fmtSize(openGame.size)}</dd></div>
        </dl>
        <div class="mt-auto pt-2">
          {#if !openGame.present}
            <p class="text-[13px] text-text-dim">This game is in the playlist but its file is not on the drive.</p>
          {:else if system?.canPlay}
            <button
              class="expo t-hover hud-corners flex h-11 w-full items-center justify-center gap-2 bg-chiral font-bold text-black transition-all hover:brightness-110 active:scale-[0.98]"
              onclick={() => play(openGame)}
            >
              <Play class="h-4 w-4 fill-black" strokeWidth={2} />
              <span class="micro font-bold">PLAY</span>
            </button>
            <p class="mt-2 text-[12px] text-text-dim">Opens on the machine the box runs on. Streaming to this screen comes later.</p>
          {:else}
            <p class="text-[13px] text-text-dim">{system?.emulator} is not on this drive, or the box is not on this machine, so this game cannot be started from here.</p>
          {/if}
        </div>
      </div>
    </div>
  </div>
{/if}
