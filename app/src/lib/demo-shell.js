// Sample data for everything outside Storage on /demo: people, AI models, the arcade
// library and clones. Like demo.js, it is the same on every visit and nothing is saved.
// Real boxes replace each of these with a call to the box; where a section has no box
// call yet, the section says it is showing sample data.

import { svgThumb } from './adapter.js';

// The demo's posters, covers, stills and game art: public domain or freely licensed, built by
// scripts/demo-media.mjs into media/ (credits: data.wearedogs.net/castle/CREDITS.md) and hosted here.
export const MEDIA_BASE = 'https://data.wearedogs.net/castle';
const hostedImages = new Map();

/** A hosted image's address once it has loaded; `fallback` when it cannot (not uploaded yet, offline). */
function hosted(file, fallback) {
  const url = `${MEDIA_BASE}/${file}`;
  if (!hostedImages.has(url)) {
    hostedImages.set(url, new Promise((done) => {
      if (typeof Image === 'undefined') return done(false);
      const img = new Image();
      img.onload = () => done(true);
      img.onerror = () => done(false);
      img.src = url;
    }));
  }
  return hostedImages.get(url).then((ok) => (ok ? url : fallback));
}

const GB = 1024 ** 3;
const MB = 1024 ** 2;
const MINUTE = 60000;
const HOUR = 60 * MINUTE;
const DAY = 24 * HOUR;
const LAUNCH_STEP_MS = 900; // how fast the pretend launch log advances
const COIN_TICK_MS = 2500;
const COINS_PER_RUN = 137;

// ---------------------------------------------------------------------------------------
// People
// ---------------------------------------------------------------------------------------

export const DEMO_PIN = '0000';

/** Roles: admin runs the box, member has a private vault, child is restricted, guest sees shared items only. */
export const DEMO_USERS = [
  { id: 'alex', name: 'Alex', role: 'Admin', tone: 'chiral', locked: false },
  { id: 'jordan', name: 'Jordan', role: 'Member', tone: 'bridge', locked: true },
  { id: 'sam', name: 'Sam', role: 'Child', tone: 'violet', locked: true },
  { id: 'guest', name: 'Guest', role: 'Guest', tone: 'faint', locked: false },
];

/** The one person a box has until it grows accounts. */
export const OWNER = { id: 'owner', name: 'Owner', role: 'Admin', tone: 'chiral', locked: false };

// ---------------------------------------------------------------------------------------
// AI
// ---------------------------------------------------------------------------------------

export const DEMO_MODELS = [
  { id: 'general-70b', name: 'General 70B', family: 'Chat', params: '70B', quant: 'Q4_K_M', size: 42.5 * GB, context: 131072, state: 'loaded', note: 'Best answers, slowest. Needs the big GPU.' },
  { id: 'coder-32b', name: 'Coder 32B', family: 'Code', params: '32B', quant: 'Q5_K_M', size: 23.3 * GB, context: 65536, state: 'ready', note: 'Writes and reviews code.' },
  { id: 'quick-12b', name: 'Quick 12B', family: 'Chat', params: '12B', quant: 'Q6_K', size: 9.8 * GB, context: 32768, state: 'ready', note: 'Fast replies for everyday questions.' },
  { id: 'tiny-4b', name: 'Tiny 4B', family: 'Chat', params: '4B', quant: 'Q8_0', size: 4.1 * GB, context: 16384, state: 'ready', note: 'Runs on anything, even without a GPU.' },
  { id: 'vision-11b', name: 'Vision 11B', family: 'Vision', params: '11B', quant: 'Q4_K_M', size: 7.9 * GB, context: 16384, state: 'available', note: 'Reads photos and screenshots.' },
  { id: 'speech-large', name: 'Speech Large', family: 'Speech', params: '1.5B', quant: 'F16', size: 3.1 * GB, context: 0, state: 'available', note: 'Turns recordings into text.' },
];

export const DEMO_AI_HARDWARE = { gpu: 'GPU 24 GB', vramUsed: 19.4 * GB, vramTotal: 24 * GB, ramUsed: 41 * GB, ramTotal: 64 * GB };

// ---------------------------------------------------------------------------------------
// Arcade: shaped like the reference drive (RetroArch playlists + standalone emulators)
// ---------------------------------------------------------------------------------------

// The sample games are real ones that are free to show: homebrew and libretro's own games whose code and art are
// under free licences. Their screenshots and box art are hosted with the site (credits: data.wearedogs.net/castle/CREDITS.md).
const SYSTEM_TINTS = [
  ['#4a1a1a', '#0e0508'], ['#3a1a4a', '#0b0e1e'], ['#1a4a2b', '#0e1e12'], ['#2b3a8a', '#0e122b'],
  ['#8a5a2b', '#2b1a0e'], ['#123a4a', '#0e0b1e'],
];
const KB = 1024;
// art: which hosted image is the game's cover (its box art, or its title screen where a game has no box);
// snap: it has an in-game screenshot. art and snap below are [hosted file, placeholder if it cannot load].
const DEMO_SYSTEM_SPECS = [
  { short: 'NES', slug: 'nes', name: 'Nintendo - Nintendo Entertainment System', emulator: 'RetroArch', games: [
    { slug: 'super-tilt-bro', title: 'Super Tilt Bro. (World)', size: 512 * KB, art: 'title', snap: true, url: 'https://github.com/sgadrat/super-tilt-bro', about: 'Super Tilt Bro. is a platform fighting game for the NES by Sylvain Gadrat, where two fighters try to knock each other off the stage. It is free software (WTFPL).' },
    { slug: 'nova-the-squirrel', title: 'Nova the Squirrel (World)', size: 256 * KB, art: 'title', snap: true, url: 'https://github.com/NovaSquirrel/NovaTheSquirrel', about: 'Nova the Squirrel is a platformer for the NES by NovaSquirrel, released as free software under the GPL.' },
    { slug: 'thwaite', title: 'Thwaite (World)', size: 32 * KB, art: 'title', snap: true, url: 'https://github.com/pinobatch/thwaite-nes', about: 'Thwaite is a missile-defence game for the NES by Damian Yerrick: aim crosshairs to shoot down missiles before they hit the houses of a small town. It is free software under the GPL.' },
    { slug: 'concentration-room', title: 'Concentration Room (World)', size: 32 * KB, art: 'title', snap: true, url: 'https://github.com/pinobatch/croom-nes', about: 'Concentration Room is a card-matching memory game for the NES by Damian Yerrick. It is free software under the GPL.' },
    { slug: 'robotfindskitten', title: 'robotfindskitten (World)', size: 16 * KB, art: 'title', snap: true, url: 'https://github.com/pinobatch/rfk-nes', about: 'robotfindskitten is a "Zen simulation": robot searches a screen full of strange objects until it finds kitten. This NES port by Damian Yerrick is free software (zlib licence).' },
    { slug: 'double-action-blaster-guys', title: 'DABG - Double Action Blaster Guys (World)', size: 64 * KB, art: 'title', snap: true, url: 'https://github.com/NovaSquirrel/DABG', about: 'Double Action Blaster Guys is an action game for the NES by NovaSquirrel, made for the 2014 NESdev competition. It is free software (zlib licence).' },
  ] },
  { short: 'GB', slug: 'gb', name: 'Nintendo - Game Boy', emulator: 'RetroArch', games: [
    { slug: 'tobu-tobu-girl', title: 'Tobu Tobu Girl (World)', size: 128 * KB, art: 'title', snap: false, url: 'https://github.com/SimonLarsen/tobutobugirl', about: 'Tobu Tobu Girl is an arcade platformer for the Game Boy by Tangram Games, bouncing ever higher into the sky. Its code (MIT) and art (CC BY 4.0) are free.' },
  ] },
  { short: 'DOOM', slug: 'doom', name: 'DOOM', emulator: 'RetroArch', games: [
    { slug: 'freedoom-phase-1', title: 'Freedoom - Phase 1', size: 28 * MB, art: 'title', snap: true, url: 'https://freedoom.github.io/', about: 'Freedoom: Phase 1 is a free game for Doom engines: four episodes in the style of the original Doom, with all-new levels, art and music under a BSD licence.' },
    { slug: 'freedoom-phase-2', title: 'Freedoom - Phase 2', size: 31 * MB, art: 'title', snap: true, url: 'https://freedoom.github.io/', about: 'Freedoom: Phase 2 is a free game for Doom engines: a 32-level campaign in the style of Doom II, with all-new levels, art and music under a BSD licence.' },
    { slug: 'freedm', title: 'Freedoom - FreeDM', size: 19 * MB, art: 'title', snap: false, url: 'https://freedoom.github.io/', about: "FreeDM is the Freedoom project's set of deathmatch levels for multiplayer Doom engines, under a BSD licence." },
  ] },
  { short: 'Dinothawr', slug: 'dinothawr', name: 'Dinothawr', emulator: 'RetroArch', games: [
    { slug: 'dinothawr', title: 'Dinothawr', size: 6 * MB, art: 'boxart', snap: true, url: 'https://github.com/libretro/Dinothawr', about: 'Dinothawr is a block-pushing puzzle game on slippery ice: a dinosaur has to free its friends, who are frozen solid. It was made for libretro; its code is GPL and its art CC BY-SA.' },
  ] },
  { short: 'Mr. Boom', slug: 'mrboom', name: 'MrBoom', emulator: 'RetroArch', games: [
    { slug: 'mr-boom', title: 'Mr.Boom', size: 3 * MB, art: 'boxart', snap: true, url: 'https://github.com/libretro/mrboom-libretro', about: 'Mr. Boom is a Bomberman-style game for up to ten players by Remdy Software, first released in 1997 and now a libretro core under the MIT licence.' },
  ] },
];

function demoGames(spec, systemIndex) {
  return spec.games.map((game, i) => {
    const tint = SYSTEM_TINTS[(i + systemIndex) % SYSTEM_TINTS.length];
    return {
      id: i,
      title: game.title,
      size: game.size,
      present: true,
      hasBoxart: !!game.art,
      hasSnap: game.snap,
      discs: [{ label: '', size: game.size, present: true }],
      art: game.art ? [`arcade/${spec.slug}/${game.art}/${game.slug}.png`, svgThumb(spec.short, tint[0], tint[1])] : null,
      snap: game.snap ? [`arcade/${spec.slug}/snap/${game.slug}.png`, svgThumb(`${spec.short} · IN GAME`, tint[1], tint[0])] : null,
      about: game.about,
      url: game.url,
    };
  });
}

/** The demo arcade, answering the same calls a box does. */
const DEMO_SAVED_GAMES = 3; // games per system that have sample saves
const DEMO_MAX_VERSIONS = 4;

// ---------------------------------------------------------------------------------------
// Steam cache: a pretend cache with made-up games (every name here is invented)
// ---------------------------------------------------------------------------------------

const STEAM_TINTS = [['#1b2838', '#0a0f16'], ['#3a2a14', '#0e0905'], ['#143a2e', '#05100c'], ['#3a1430', '#0e050c'], ['#24304a', '#070a12']];
const DEMO_STEAM_GAMES = [
  { app: 912340, name: 'Skyforge Tactics', full: 38.2 * GB, cached: 38.2 * GB, state: 'cached', dev: 'North Wind Games', genres: ['Strategy'], released: 'Mar 4, 2024', desc: 'Command an airship fleet across a broken sky. Turn-based battles, a campaign of 40 missions.' },
  { app: 734560, name: 'Harbor Lights', full: 6.4 * GB, cached: 6.4 * GB, state: 'update', dev: 'Lantern House', genres: ['Adventure', 'Indie'], released: 'Oct 19, 2022', desc: 'Run a lighthouse on a stormy coast and keep the ships off the rocks, one night at a time.' },
  { app: 556120, name: 'Iron Valley Racing', full: 61.0 * GB, cached: 23.5 * GB, state: 'partial', dev: 'Gearbox Bay', genres: ['Racing', 'Sports'], released: 'Jun 1, 2025', desc: 'Off-road racing through a mountain valley that changes with the seasons.' },
  { app: 401230, name: 'Pocket Kingdom', full: 2.1 * GB, cached: 0, state: 'empty', dev: 'Tiny Crown', genres: ['Simulation'], released: 'Jan 12, 2021', desc: 'Build a kingdom the size of a garden. Your citizens are very small and very opinionated.' },
];

class DemoSteam {
  constructor() {
    const now = Date.now();
    this.games = DEMO_STEAM_GAMES.map((g, i) => ({
      app: g.app, name: g.name, header: true, cached: g.cached, full: g.full, state: g.state,
      updated: now - (i * 11 + 3) * DAY, buildid: String(1880000 + i * 3721), lastServed: g.cached ? now - (i + 1) * 5 * HOUR : 0,
      kept: i !== 2, installedHere: i < 2, saved: g.cached * (2 - i * 0.5 > 0 ? 2 - i * 0.5 : 0),
      depots: [{ depot: g.app + 1, cached: g.cached, chunks: Math.round(g.cached / MB), manifests: 1, current: String(7200000000000000000 + i), upToDate: g.state !== 'update', size: g.full }],
      description: g.desc, developers: [g.dev], genres: g.genres, released: g.released,
    }));
    this.installed = [
      { app: 912340, name: 'Skyforge Tactics', size: 38.2 * GB, updated: now - 3 * DAY, buildid: '1880000', depots: {}, library: 'C:\\Program Files (x86)\\Steam' },
      { app: 734560, name: 'Harbor Lights', size: 6.4 * GB, updated: now - 14 * DAY, buildid: '1883721', depots: {}, library: 'C:\\Program Files (x86)\\Steam' },
      { app: 288880, name: 'Moonwell', size: 11.7 * GB, updated: now - 40 * DAY, buildid: '991203', depots: {}, library: 'D:\\SteamLibrary' },
    ];
  }

  async overview() {
    const sum = (key) => this.games.reduce((total, g) => total + g[key], 0);
    return {
      listening: '0.0.0.0:80', dns: '0.0.0.0:53', boxAddress: '192.168.1.50', cacheDir: 'D:\\SteamCache',
      steamPointsHere: true, resolved: ['192.168.1.50'], canTurnOn: true, installedHere: this.installed.length,
      totals: { bytes: sum('cached'), chunks: Math.round(sum('cached') / MB), hits: 81234, misses: 52011, saved: sum('saved'), free: 1.2 * 1024 * GB },
      apps: structuredClone(this.games),
      downloading: [{ client: '192.168.1.31', app: 556120, name: 'Iron Valley Racing', bytes: 1.6 * GB, hitPercent: 38, mbps: 92.4 }],
    };
  }

  async art(app) {
    const game = this.games.find((g) => g.app === app) || { name: `App ${app}` };
    const [a, b] = STEAM_TINTS[app % STEAM_TINTS.length];
    return svgThumb(game.name.toUpperCase(), a, b);
  }

  async search(q) {
    const words = ['Lantern', 'Atlas', 'Frontier', 'Echo', 'Granite'];
    return words.map((w, i) => ({ app: 600000 + i * 10, name: `${q.trim() || 'Game'}: ${w} Edition`, image: '' }));
  }

  async local() {
    return structuredClone(this.installed);
  }

  async keep(app, keep) {
    const game = this.games.find((g) => g.app === app);
    if (game) game.kept = keep;
    else if (keep) {
      const found = this.installed.find((g) => g.app === app);
      this.games.push({ app, name: found?.name || `App ${app}`, header: false, cached: 0, full: found?.size || 0, state: 'empty', updated: Date.now(), buildid: '', lastServed: 0, kept: true, installedHere: !!found, saved: 0, depots: [], description: '', developers: [], genres: [], released: '' });
    }
  }

  async purge(app) {
    const game = this.games.find((g) => g.app === app);
    const freed = game?.cached || 0;
    this.games = this.games.filter((g) => g.app !== app);
    return { freed };
  }
}

export class DemoArcade {
  steam = new DemoSteam();
  steamCache() { return this.steam.overview(); }
  steamArt(app) { return this.steam.art(app); }
  steamSearch(q) { return this.steam.search(q); }
  steamLocal() { return this.steam.local(); }
  steamKeep(app, keep = true) { return this.steam.keep(app, keep); }
  steamPurge(app) { return this.steam.purge(app); }
  async steamRefresh() {}
  async steamEnable() { throw new Error('Demo: the cache is already on'); }
  async steamDnsEnable() { throw new Error('Demo: nothing to turn on'); }

  isDemo = true;

  constructor() {
    this.library = DEMO_SYSTEM_SPECS.map((spec, i) => ({ spec, games: demoGames(spec, i) }));
  }

  async arcadeSystems() {
    return this.library.map(({ spec, games }) => ({
      id: spec.name,
      name: spec.name,
      short: spec.short,
      emulator: spec.emulator,
      games: games.length,
      withArt: games.filter((g) => g.hasBoxart).length,
      bytes: games.reduce((sum, g) => sum + g.size, 0),
      canPlay: true,
    }));
  }

  async arcadeGames(system) {
    return this.library.find((s) => s.spec.name === system)?.games || [];
  }

  async arcadeArt(system, game, kind = 'boxart') {
    const found = (await this.arcadeGames(system))[game];
    if (kind === 'snap') return found?.hasSnap ? hosted(...found.snap) : null;
    return found?.hasBoxart ? hosted(...found.art) : null;
  }

  async arcadePlay(system, game, disc = 0) {
    const found = (await this.arcadeGames(system))[game];
    const label = found?.discs?.[disc]?.label;
    return { started: `${found?.title || ''}${label ? ` (${label})` : ''}`, demo: true };
  }

  /** What the game's own project says about it, as a real box's card shows Wikipedia's. */
  async arcadeAbout(system, game) {
    const found = (await this.arcadeGames(system))[game];
    if (!found) return null;
    return { title: found.title.replace(/ \(.*$/, ''), extract: found.about, url: found.url };
  }

  /** Sample saves: the first few games of each system, some with a save state as well. */
  demoSaves() {
    if (this.saves) return this.saves;
    const now = Date.now();
    this.saves = [];
    this.versions = new Map();
    this.library.forEach(({ spec, games }, s) => {
      games.slice(0, DEMO_SAVED_GAMES).forEach((game, g) => {
        const kinds = g % 2 ? ['battery', 'state'] : ['battery'];
        for (const kind of kinds) {
          const name = `${game.title}.${kind === 'battery' ? 'srm' : 'state'}`;
          const id = `Consoles/RetroArch-Win64/${kind === 'battery' ? 'saves' : 'states'}/${spec.emulator}/${name}`;
          const count = 1 + ((s + g) % DEMO_MAX_VERSIONS);
          const size = kind === 'battery' ? 32 * 1024 : 2 * MB;
          const versions = Array.from({ length: count }, (_, v) => ({ seq: this.versions.size * 10 + v, at: now - (v * 3 + g + 1) * DAY, save: id, hash: demoHash(s * 100 + g * 10 + v), size, modified: now - (v * 3 + g + 1) * DAY - HOUR }));
          this.versions.set(id, versions);
          this.saves.push({ id, name, core: spec.emulator, kind, game: { system: spec.name, id: game.id, title: game.title }, size, modified: versions[0].modified, versions: count, recorded: true });
        }
      });
    });
    this.saves.sort((a, b) => b.modified - a.modified);
    return this.saves;
  }

  async arcadeSaves() {
    return this.demoSaves();
  }

  async arcadeSavesRecord() {
    return { checked: this.demoSaves().length, added: 0 };
  }

  async arcadeSaveHistory(id) {
    this.demoSaves();
    return this.versions.get(id) || [];
  }

  async arcadeSaveRestore(id, hash) {
    const versions = this.versions.get(id) || [];
    const chosen = versions.find((v) => v.hash === hash);
    if (!chosen) throw new Error('that version was never recorded for this save');
    versions.unshift({ ...chosen, seq: chosen.seq + 1000, at: Date.now(), modified: Date.now() });
    const save = this.saves.find((s) => s.id === id);
    save.versions = versions.length;
    save.modified = Date.now();
    return { restored: true };
  }
}

// ---------------------------------------------------------------------------------------
// Clones
// ---------------------------------------------------------------------------------------

const DEMO_LAUNCH_LOG = [
  '  [PROFILE] penguin -- Cart Surfer bot on Club Penguin Journey',
  '  [PAYLOAD] packed penguin-autosurf (as git sees it) + .env',
  '  [DISK] debian-x11.qcow2 found -- booting the built VM',
  '  [*] QEMU window opened: 4 GB, 8 cores, 1440x900',
  '  [GUEST] agent is up -- copying the payload',
  '  [SETUP] unpacking, building the Python environment',
  '  [SETUP] Firefox is on the start screen',
  '[+] The bot is playing Cart Surfer in the VM. F8 pauses, F9 quits.',
];

const DEMO_TARGETS = [
  { id: 'debian-x11', name: 'Debian 13 (minimal X11 desktop)', description: 'Light desktop for automations.', family: 'linux', runtime: '', cloud: true, inject: true, built: true, disks: 1 },
  { id: 'container-debian', name: 'Debian 13 container (light, for automations)', description: 'An in-memory screen and Openbox in a Docker container: up in seconds, only the memory its programs use.', family: 'linux', runtime: 'container', cloud: false, inject: true, automatable: true, built: false, disks: 0 },
  { id: 'container-ubuntu', name: 'Ubuntu 24.04 container', description: 'Ubuntu 24.04 LTS in a Docker container, set up like the Debian one.', family: 'linux', runtime: 'container', cloud: false, inject: true, automatable: true, built: false, disks: 0 },
  { id: 'container-arch', name: 'Arch Linux container', description: 'Arch Linux in a Docker container, with pacman.', family: 'linux', runtime: 'container', cloud: false, inject: true, automatable: true, built: false, disks: 0 },
  { id: 'container-fedora', name: 'Fedora 45 container', description: 'Fedora 45 in a Docker container, with dnf.', family: 'linux', runtime: 'container', cloud: false, inject: true, automatable: true, built: false, disks: 0 },
  { id: 'container-android', name: 'Android 15 container (Google emulator, headless)', description: 'The Android emulator in a Docker container. Installs your APKs; Android automations run their tasks on the phone.', family: 'android', runtime: 'container', apk: true, cloud: false, inject: false, automatable: true, built: false, disks: 0 },
  { id: 'endeavouros', name: 'EndeavourOS Linux', description: 'Friendly Arch-based distribution.', family: 'linux', runtime: '', cloud: false, inject: true, built: true, disks: 1 },
  { id: 'cachyos', name: 'CachyOS Linux', description: 'Performance-tuned Arch-based distribution.', family: 'linux', runtime: '', cloud: false, inject: false, built: false, disks: 0 },
  { id: 'ubuntu-cloud', name: 'Ubuntu 24.04 LTS (headless)', description: 'SSH-ready in a minute.', family: 'linux', runtime: '', cloud: true, inject: false, built: false, disks: 0 },
  { id: 'win11-pro', name: 'Windows 11 Pro', description: 'Unattended install.', family: 'windows', runtime: '', cloud: false, inject: false, built: true, disks: 1 },
  { id: 'android-avd', name: 'Android 15 (Google emulator)', description: 'Play Store image. Installs the APKs you give it.', family: 'android', runtime: 'avd', apk: true, cloud: false, inject: false, automatable: true, built: false, disks: 0 },
  { id: 'blissos', name: 'BlissOS 16 (Android 13)', description: 'Phone apps on x86.', family: 'android', runtime: '', cloud: false, inject: false, built: false, disks: 0 },
  { id: 'mac', name: 'macOS Bootstrap (OpenCore)', description: 'Bare-metal bootstrap shim.', family: 'macos', runtime: '', cloud: false, inject: false, built: false, disks: 0 },
];

const DEMO_FIELDS = [
  { env: 'PENGUIN_USERNAME', label: 'Username', secret: false },
  { env: 'PENGUIN_PASSWORD', label: 'Password', secret: true },
];

/** A pretend box for Clones: one example automation whose bots walk through a launch, a
    small catalogue, and machines that start and stop at once. Nothing real is started. */
export class DemoClones {
  isDemo = true;

  constructor() {
    this.automations = [{
      id: 'penguin', name: 'Penguin AutoSurf', description: 'Signs in to Club Penguin Journey and plays Cart Surfer on a loop for coins.',
      target: 'debian-x11', source: '../../penguin-autosurf', sourceTrusted: true, repoPresent: true, memoryGb: 4, cores: 8, screen: '1440x900',
      apt: [], pacman: [], tasks: [], fields: DEMO_FIELDS, restart: false, stopAfterMinutes: 0, temporary: false, builtin: true,
      bots: [{ id: 'main', name: 'Penguin 1', saved: {}, startedAt: 0, state: 'stopped' }],
    }];
    this.machines = DEMO_TARGETS.filter((t) => t.built).map((t) => ({ id: t.id, key: `m:${t.id}`, name: t.name, target: t.id, kind: 'main', state: 'stopped', diskBytes: 6.2 * GB, log: [] }));
    this.bench = [
      { at: Date.now() - 2 * HOUR, label: 'Debian 13 (minimal X11 desktop)', target: 'debian-x11', mode: 'existing', firstBoot: false, prepSeconds: 1.4, readySeconds: 11.8, ok: true },
      { at: Date.now() - 26 * HOUR, label: 'Debian 13 (minimal X11 desktop)', target: 'debian-x11', mode: 'new', firstBoot: true, prepSeconds: 38.2, readySeconds: 214.5, ok: true },
    ];
  }

  /** A bot's state and log by wall-clock time, so polling shows a launch moving along. */
  botView(a, bot, index) {
    let log = [];
    let state = bot.state;
    if (state !== 'stopped') {
      const elapsed = Date.now() - bot.startedAt;
      const shown = Math.min(DEMO_LAUNCH_LOG.length, Math.floor(elapsed / LAUNCH_STEP_MS) + 1);
      log = DEMO_LAUNCH_LOG.slice(0, shown);
      state = shown < DEMO_LAUNCH_LOG.length ? 'launching' : 'running';
      const runs = Math.floor((elapsed - DEMO_LAUNCH_LOG.length * LAUNCH_STEP_MS) / COIN_TICK_MS);
      for (let run = 1; run <= Math.min(runs, 6); run++) log.push(`cart game ${run} finished: +${COINS_PER_RUN} coins (${run * COINS_PER_RUN} this session)`);
    }
    const values = a.fields.map((f) => ({ env: f.env, from: bot.saved[f.env] ? 'saved' : 'missing', value: f.secret ? '' : bot.saved[f.env] || '' }));
    const runtime = DEMO_TARGETS.find((t) => t.id === a.target)?.runtime;
    const engine = runtime === 'container' ? 'container' : runtime === 'avd' ? 'emulator' : 'vm';
    const family = DEMO_TARGETS.find((t) => t.id === a.target)?.family || 'linux';
    return { id: bot.id, name: bot.name, key: `a:${a.id}:${bot.id}`, state, ready: values.every((v) => v.from !== 'missing'), ownDisk: index > 0 || a.temporary || engine !== 'vm', engine, controllable: engine === 'container' && family === 'linux', values, log };
  }

  view(a) {
    const bots = a.bots.map((bot, index) => this.botView(a, bot, index));
    const state = bots.some((b) => b.state === 'running') ? 'running' : bots.some((b) => b.state === 'launching') ? 'launching' : 'stopped';
    return { ...a, state, bots };
  }

  find(id) {
    const a = this.automations.find((x) => x.id === id);
    if (!a) throw new Error('unknown automation');
    return a;
  }

  async clones() {
    return this.automations.map((a) => this.view(a));
  }

  async cloneAutomationSave(def) {
    if (!def.name?.trim()) throw new Error('give the automation a name');
    let a = this.automations.find((x) => x.id === def.id);
    if (a) Object.assign(a, def, { bots: a.bots, builtin: a.builtin });
    else {
      a = { ...def, id: `demo-${this.automations.length + 1}`, builtin: false, sourceTrusted: true, repoPresent: true, bots: [{ id: 'main', name: `${def.name} 1`, saved: {}, startedAt: 0, state: 'stopped' }] };
      this.automations.push(a);
    }
    return this.view(a);
  }

  async cloneAutomationDelete(id) {
    if (this.find(id).builtin) throw new Error('the built-in example cannot be removed');
    this.automations = this.automations.filter((a) => a.id !== id);
    return this.clones();
  }

  async cloneBotSave(id, botId, name, values) {
    const a = this.find(id);
    let bot = a.bots.find((b) => b.id === botId);
    if (!bot) {
      bot = { id: `b${a.bots.length + 1}`, name: `${a.name} ${a.bots.length + 1}`, saved: {}, startedAt: 0, state: 'stopped' };
      a.bots.push(bot);
    }
    if (name?.trim()) bot.name = name.trim();
    for (const [env, value] of Object.entries(values || {})) {
      if (value.trim()) bot.saved[env] = value.trim();
      else if (value) delete bot.saved[env];
    }
    return this.view(a);
  }

  async cloneBotDelete(id, botId) {
    const a = this.find(id);
    a.bots = a.bots.filter((b) => b.id !== botId);
    return this.view(a);
  }

  async cloneStart(id, botId) {
    const a = this.find(id);
    const bot = a.bots.find((b) => b.id === (botId || 'main'));
    if (!bot) throw new Error('unknown bot');
    if (bot.state !== 'stopped') throw new Error('it is already running');
    bot.state = 'launching';
    bot.startedAt = Date.now();
    return this.view(a);
  }

  async cloneStop(id, botId) {
    const a = this.find(id);
    const bot = a.bots.find((b) => b.id === (botId || 'main'));
    if (bot) bot.state = 'stopped';
    return this.view(a);
  }

  async cloneLive() {
    throw new Error('Taking control needs a real box: the demo has no containers');
  }

  async cloneMachines() {
    this.machines = this.machines.filter((m) => m.kind !== 'temporary' || m.state === 'running');
    const engine = (m) => {
      const runtime = DEMO_TARGETS.find((t) => t.id === m.target)?.runtime;
      return runtime === 'container' ? 'container' : runtime === 'avd' ? 'emulator' : 'vm';
    };
    const machines = this.machines.map((m) => ({ ...m, engine: engine(m), controllable: engine(m) === 'container' && !m.target.includes('android') }));
    return { targets: DEMO_TARGETS, machines, bench: this.bench, docker: true };
  }

  async cloneMachineStart(body) {
    let machine = this.machines.find((m) => m.id === body.machine);
    if (body.mode !== 'existing') {
      const target = DEMO_TARGETS.find((t) => t.id === body.target);
      const temporary = body.mode === 'temporary';
      const id = temporary ? `tmp-${Date.now()}` : body.name.trim().toLowerCase().replace(/[^a-z0-9]+/g, '-');
      if (!id) throw new Error('give the new machine a name with letters or digits in it');
      machine = { id, key: `m:${id}`, name: temporary ? `${target.name} (temporary)` : body.name.trim(), target: target.id, kind: temporary ? 'temporary' : 'instance', state: 'stopped', diskBytes: 0.2 * GB, log: [] };
      this.machines.push(machine);
    }
    if (!machine) throw new Error('unknown machine');
    machine.state = 'running';
    machine.log = ['  [DEMO] A pretend machine: nothing was started.'];
    this.bench.unshift({ at: Date.now(), label: machine.name, target: machine.target, mode: body.mode, firstBoot: false, prepSeconds: 1.2, readySeconds: 9.6, ok: true });
    return this.cloneMachines();
  }

  async cloneMachineStop(id) {
    const machine = this.machines.find((m) => m.id === id);
    if (machine) machine.state = 'stopped';
    return this.cloneMachines();
  }

  async cloneMachineDelete(id) {
    this.machines = this.machines.filter((m) => m.id !== id);
    return this.cloneMachines();
  }
}

/** Skills an automation has or will have. `ready: false` ones are shown, not clickable. */
export const AUTOMATION_SKILLS = {
  penguin: [
    { id: 'cart-surfer', name: 'Cart Surfer', detail: 'Mine cart runs for coins', ready: true },
    { id: 'card-jitsu', name: 'Card-Jitsu', detail: 'Karate belts in the Dojo', ready: false },
  ],
};

/** Automations that are designed but not built. They show as cards with no controls. */
export const PLANNED_AUTOMATIONS = [
  { id: 'idleon', name: 'Idleon', description: 'Idle-game upkeep: claim, craft and re-queue on a schedule.', skills: ['AFK claims', 'Crafting queues'] },
];

/** VM targets as the demo shows them. */
export const DEMO_MACHINES = [
  { id: 'debian-x11', name: 'Debian X11', detail: 'Light desktop for automations', state: 'running', cores: 8, memoryGb: 4, diskBytes: 6.2 * GB, uptime: 3 * HOUR + 12 * MINUTE },
  { id: 'cachyos', name: 'CachyOS', detail: 'Arch-based daily driver', state: 'stopped', cores: 8, memoryGb: 8, diskBytes: 38 * GB, uptime: 0 },
  { id: 'ubuntu-cloud', name: 'Ubuntu Server', detail: 'Headless, SSH-ready in a minute', state: 'stopped', cores: 4, memoryGb: 4, diskBytes: 3.4 * GB, uptime: 0 },
  { id: 'win11-pro', name: 'Windows 11 Pro', detail: 'Unattended install', state: 'stopped', cores: 8, memoryGb: 8, diskBytes: 41 * GB, uptime: 0 },
  { id: 'android', name: 'Android (Bliss)', detail: 'Phone apps on x86', state: 'not built', cores: 4, memoryGb: 4, diskBytes: 0, uptime: 0 },
];

// ---------------------------------------------------------------------------------------
// Network
// ---------------------------------------------------------------------------------------

const DEMO_DOWN_BYTES_PER_SEC = 4.2 * MB; // what the pretend connection idles around
const DEMO_UP_BYTES_PER_SEC = 0.6 * MB;
const DEMO_WAVE_MS = 9000; // how long one swell of pretend traffic lasts
const DEMO_SPEED_TEST_MS = 3200;
const DEMO_START_RX = 184 * GB;
const DEMO_START_TX = 22 * GB;

/** A pretend connection: counters that climb in swells, a canned speed test and provider. */
export class DemoNetwork {
  isDemo = true;

  constructor() {
    this.startedAt = Date.now();
    this.tests = 0;
  }

  async network() {
    const seconds = (Date.now() - this.startedAt) / 1000;
    // The integral of a rate that swells and settles, so two readings always differ smoothly.
    const swell = (DEMO_WAVE_MS / 1000 / (2 * Math.PI)) * (1 - Math.cos((2 * Math.PI * seconds * 1000) / DEMO_WAVE_MS));
    return {
      at: Date.now(),
      interfaces: [
        { name: 'Ethernet', rxBytes: Math.round(DEMO_START_RX + DEMO_DOWN_BYTES_PER_SEC * (seconds + 0.8 * swell)), txBytes: Math.round(DEMO_START_TX + DEMO_UP_BYTES_PER_SEC * (seconds + 0.5 * swell)) },
        { name: 'Wi-Fi', rxBytes: Math.round(3.1 * GB + 0.05 * MB * seconds), txBytes: Math.round(0.4 * GB + 0.01 * MB * seconds) },
      ],
      local: { adapter: 'Ethernet', address: '192.168.1.20', gateway: '192.168.1.1', dns: ['192.168.1.1'] },
    };
  }

  async networkSpeedTest() {
    await new Promise((done) => setTimeout(done, DEMO_SPEED_TEST_MS));
    this.tests += 1;
    const wobble = (this.tests * 37) % 40;
    return { at: Date.now(), downMbps: 912.4 - wobble, upMbps: 38.6 - wobble / 20, latencyMs: 11.2 + wobble / 10, downloadBytes: 50_000_000, uploadBytes: 10_000_000 };
  }

  async networkProvider() {
    return { address: '203.0.113.42', provider: 'Example Fiber', asn: 'AS64500', city: 'Springfield', region: 'Example State', country: 'US', timezone: 'America/New_York' };
  }
}

/** Devices as the demo's Network section lists them. A real box cannot see these until it is the router. */
export const DEMO_DEVICES = [
  { id: 'd1', name: 'Living-room TV', kind: 'tv', address: '192.168.1.31', downBytes: 61 * GB, upBytes: 1.2 * GB, online: true },
  { id: 'd2', name: 'Gaming PC', kind: 'laptop', address: '192.168.1.20', downBytes: 48 * GB, upBytes: 9.4 * GB, online: true },
  { id: 'd3', name: 'Alex\'s phone', kind: 'phone', address: '192.168.1.44', downBytes: 7.3 * GB, upBytes: 0.9 * GB, online: true },
  { id: 'd4', name: 'Jordan\'s laptop', kind: 'laptop', address: '192.168.1.52', downBytes: 12 * GB, upBytes: 2.1 * GB, online: false },
  { id: 'd5', name: 'Doorbell camera', kind: 'tv', address: '192.168.1.77', downBytes: 0.2 * GB, upBytes: 14 * GB, online: true },
];

export const DEMO_DNS = { queriesToday: 48213, blockedToday: 9127, topBlocked: ['ads.example-network.com', 'tracker.example-metrics.net', 'telemetry.example-tv.com'] };

// ---------------------------------------------------------------------------------------
// Box settings and facts
// ---------------------------------------------------------------------------------------

/** Box settings held in memory, and the facts a box reports about itself. */
const DEMO_BOX_STARTED = Date.now() - 12 * DAY - 7 * HOUR;
const DEMO_CPU_WAVE_MS = 9000;

export class DemoBox {
  isDemo = true;

  /** Vital signs that drift gently, so the gauges move like a real box's. */
  async system() {
    const wave = Math.sin((Date.now() / DEMO_CPU_WAVE_MS) * 2 * Math.PI);
    return {
      at: Date.now(),
      host: 'castle',
      os: 'Debian GNU/Linux 13',
      uptimeSeconds: Math.round((Date.now() - DEMO_BOX_STARTED) / 1000),
      uptimeMillis: Date.now() - DEMO_BOX_STARTED,
      cores: 4,
      cpuPercent: 22 + 14 * wave,
      memoryUsed: (2.6 + 0.3 * wave) * GB,
      memoryTotal: 8 * GB,
      diskFree: 1479 * GB,
      diskTotal: 2000 * GB,
    };
  }

  constructor() {
    this.saved = {};
  }

  async settings() {
    return { ...this.saved };
  }

  async saveSettings(changes) {
    Object.assign(this.saved, changes);
    return { ...this.saved };
  }

  async boxInfo() {
    return {
      version: '0.1.0',
      listen: '127.0.0.1:8787',
      loopback: true,
      clones: true,
      arcade: true,
      links: {
        tailscale: { installed: true, state: 'Running', name: 'castle.example-tailnet.ts.net', addresses: ['100.64.0.12'], tailnet: 'example-tailnet' },
        wireguardInstalled: false,
      },
    };
  }
}

// ---- Security ----

/** Protections Castle does not have yet, shown as such in Security > Not built yet. */
export const PLANNED_PROTECTIONS = [
  { title: 'Accounts and strong sign-in', what: 'A separate login for each person, with a passkey or a second step, and a key for each device so one lost phone can be shut out on its own.' },
  { title: 'Vault encryption', what: 'Castle encrypting files itself, per person, so a stolen disk or another account on the box reads nothing.' },
  { title: 'Record held on your other devices', what: 'Each Castle device keeps a copy of the newest fingerprint of the others\' activity records, so a rewritten record is noticed.' },
  { title: 'File fingerprints', what: 'A stored fingerprint of every file, re-checked on a schedule, so a file changed outside Castle is reported with when it was last known good.' },
  { title: 'Watching other devices', what: 'Seeing every device on the network and what it talks to. This needs Castle to be the router.' },
  { title: 'Emergency intake', what: 'Receiving a disk image from a machine you suspect is infected, and proving later that the image was not altered. Phoenix already has the sending side.' },
  { title: 'Windows audits', what: 'The Phoenix listings for Windows machines (startup items, drivers, scheduled tasks, firewall rules), run from Castle against your PCs.' },
  { title: 'Alerts', what: 'Telling you when a check starts failing or the record breaks, instead of waiting for you to open this screen.' },
];

const DEMO_HASH_CHARS = 64;
/** A stable, made-up fingerprint for sample entry `n`. */
const demoHash = (n) => Array.from({ length: DEMO_HASH_CHARS }, (_, i) => ((n * 7 + i * 13 + ((n * i) % 11)) % 16).toString(16)).join('');
const DEMO_ACTIVITY = [
  ['box.start', 'castled 0.1.0 on 127.0.0.1:8787', 26 * HOUR],
  ['upload', '200 parent=photos&name=harbour.jpg', 25 * HOUR],
  ['mkdir', '200 name=Taxes 2026 parent=documents', 22 * HOUR],
  ['rename', '200 id=documents/scan0001.pdf name=passport.pdf', 21 * HOUR],
  ['access.denied', '/list', 9 * HOUR],
  ['trash', '204 id=downloads/installer.exe', 5 * HOUR],
  ['clones.start', '200 id=penguin', 3 * HOUR],
  ['settings', '200', 2 * HOUR],
  ['network.speedtest', '200', 40 * 60 * 1000],
];
const DEMO_AUDITS = [
  { id: 'users', label: 'Accounts', about: 'Every account on the box, its groups, and whether it can log in' },
  { id: 'listening', label: 'Open ports', about: 'What is listening for connections, and whether other machines can reach it' },
  { id: 'connections', label: 'Connections', about: 'Established connections, split into the outside world and the local network' },
  { id: 'services', label: 'Running services', about: 'Services running right now' },
  { id: 'startup', label: 'Starts with the box', about: 'Everything set to start on its own: enabled services, cron, autostart entries' },
  { id: 'tasks', label: 'Scheduled tasks', about: 'Timers and cron jobs, and what each runs' },
];
const DEMO_AUDIT_TABLES = {
  users: [['USER', 'UID', 'CAN LOG IN', 'GROUPS', 'SHELL'], ['root', '0', 'yes', 'root', '/bin/bash'], ['castle', '1000', 'yes', 'castle,sudo', '/bin/bash'], ['www-data', '33', 'no', 'www-data', '/usr/sbin/nologin']],
  listening: [['PROTOCOL', 'ADDRESS', 'PORT', 'REACHABLE FROM', 'PROCESS'], ['tcp', '0.0.0.0', '22', 'other machines', 'sshd'], ['tcp', '0.0.0.0', '445', 'other machines', 'smbd'], ['tcp', '127.0.0.1', '8787', 'this box only', 'castled'], ['udp', '0.0.0.0', '51820', 'other machines', '-']],
  connections: [['GOES TO', 'REMOTE ADDRESS', 'PORT', 'PROCESS'], ['outside world', '203.0.113.10', '443', 'tailscaled'], ['local network', '192.168.1.42', '51234', 'smbd']],
  services: [['SERVICE', 'STATE', 'WHAT IT IS'], ['castled.service', 'running', 'Castle box daemon'], ['smbd.service', 'running', 'Samba file sharing'], ['ssh.service', 'running', 'OpenSSH server']],
  startup: [['WHERE', 'NAME', 'RUNS'], ['systemd', 'castled.service', '/usr/local/bin/castled --data /srv/castle'], ['systemd', 'smbd.service', '/usr/sbin/smbd --foreground']],
  tasks: [['KIND', 'NAME', 'WHEN', 'RUNS'], ['timer (enabled)', 'apt-daily.timer', 'OnCalendar=*-*-* 06,18:00:00', 'apt-daily.service'], ['cron', '/etc/cron.d/castle-backup', '0 3 * * *', '/usr/local/bin/castle-backup']],
};
const DEMO_AUDIT_MS = 600;

/** A pretend box's security state: a fixed set of checks, a short record, canned audits. */
export class DemoSecurity {
  isDemo = true;

  constructor() {
    const now = Date.now();
    this.startedAt = now - DEMO_ACTIVITY[0][2];
    this.tokenMadeAt = now - 30 * DAY;
    this.entries = DEMO_ACTIVITY.map(([action, detail, ago], i) => ({ seq: i + 1, at: now - ago, action, detail, prev: demoHash(i), hash: demoHash(i + 1) }));
  }

  record() {
    return { intact: true, entries: this.entries.length, head: this.entries.at(-1).hash, brokenAt: null };
  }

  async security() {
    return {
      checks: [
        { id: 'listen', label: 'Who can reach the box', state: 'ok', detail: 'Only this computer (127.0.0.1:8787)' },
        { id: 'link', label: 'Private link', state: 'ok', detail: 'Tailscale is connected' },
        { id: 'record', label: 'Activity record', state: 'ok', detail: `${this.entries.length} entries, every one follows from the one before` },
        { id: 'denied', label: 'Refused calls', state: 'warn', detail: '1 call with a wrong or missing token in the last 24 hours' },
        { id: 'token-file', label: 'Token file', state: 'ok', detail: "Only the box's own account can read it." },
        { id: 'disk', label: 'Disk encryption', state: 'warn', detail: '/dev/sda2 is not encrypted. Anyone holding the disk can read the vault.' },
        { id: 'accounts', label: 'Accounts', state: 'off', detail: 'One token opens everything. Per-person accounts are not built yet.' },
        { id: 'vault-encryption', label: 'Vault encryption', state: 'off', detail: "Castle does not encrypt files itself yet; it relies on the disk's encryption." },
        { id: 'witness', label: 'Record held elsewhere', state: 'off', detail: "The record's newest hash is not copied to another device yet, so a full rewrite would go unnoticed." },
      ],
      record: this.record(),
      deniedLastDay: 1,
      startedAt: this.startedAt,
      tokenMadeAt: this.tokenMadeAt,
    };
  }

  async securityLog(limit) {
    return [...this.entries].reverse().slice(0, limit);
  }

  async securityVerify() {
    return this.record();
  }

  async securityRotate() {
    this.tokenMadeAt = Date.now();
    const last = this.entries.at(-1);
    this.entries.push({ seq: last.seq + 1, at: Date.now(), action: 'security.token.rotate', detail: '200', prev: last.hash, hash: demoHash(last.seq + 1) });
  }

  async securityAudits() {
    return { available: true, reason: '', audits: DEMO_AUDITS };
  }

  async securityAudit(id) {
    await new Promise((done) => setTimeout(done, DEMO_AUDIT_MS));
    const [columns, ...rows] = DEMO_AUDIT_TABLES[id];
    return { id, at: Date.now(), columns, rows, limited: false };
  }
}

// ---------------------------------------------------------------------------------------
// Theater: real films, shows and albums that are free to show: public-domain films and cartoons, the Blender
// open movies, Creative Commons music and photos. Their posters, stills and covers are hosted with the site
// (credits: data.wearedogs.net/castle/CREDITS.md). Paths, sizes, people's photo folders and home videos are made up.
// ---------------------------------------------------------------------------------------

const DEMO_TINTS = [
  ['#5a1a2b', '#0e0508'], ['#1a3a5a', '#05080e'], ['#2b5a1a', '#080e05'], ['#5a4a1a', '#0e0b05'],
  ['#3a1a5a', '#08050e'], ['#1a5a52', '#050e0d'], ['#5a2b1a', '#0e0805'], ['#24245a', '#05050e'],
];
const DEMO_FILMS = [
  { slug: 'metropolis', title: 'Metropolis', year: 1927, genres: ['Science fiction', 'Drama'], minutes: 153, cast: ['Brigitte Helm', 'Gustav Fröhlich', 'Alfred Abel', 'Rudolf Klein-Rogge'], director: 'Fritz Lang', overview: "In a towering city of the future, the son of the city's master falls for a woman who speaks for the workers below, while an inventor builds a machine in her likeness." },
  { slug: 'nosferatu', title: 'Nosferatu', year: 1922, genres: ['Horror'], minutes: 94, cast: ['Max Schreck', 'Gustav von Wangenheim', 'Greta Schröder'], director: 'F. W. Murnau', overview: 'An estate agent travels to the Carpathian mountains to sell a house to Count Orlok, who follows him home and brings death with him.' },
  { slug: 'the-general', title: 'The General', year: 1926, genres: ['Comedy', 'Action'], minutes: 78, cast: ['Buster Keaton', 'Marion Mack'], director: 'Buster Keaton, Clyde Bruckman', overview: 'When Union spies steal his locomotive with his sweetheart aboard, a Southern railway engineer chases them alone.' },
  { slug: 'sherlock-jr', title: 'Sherlock Jr.', year: 1924, genres: ['Comedy'], minutes: 45, cast: ['Buster Keaton', 'Kathryn McGuire'], director: 'Buster Keaton', overview: 'A cinema projectionist who wants to be a detective is framed for a theft, falls asleep at work and dreams himself into the film on the screen.' },
  { slug: 'the-kid', title: 'The Kid', year: 1921, genres: ['Comedy', 'Drama'], minutes: 68, cast: ['Charlie Chaplin', 'Jackie Coogan', 'Edna Purviance'], director: 'Charlie Chaplin', overview: 'The Tramp finds an abandoned baby and raises him, until the authorities come to take the boy away.' },
  { slug: 'the-gold-rush', title: 'The Gold Rush', year: 1925, genres: ['Comedy', 'Adventure'], minutes: 95, cast: ['Charlie Chaplin', 'Georgia Hale', 'Mack Swain'], director: 'Charlie Chaplin', overview: 'The Lone Prospector heads to the Klondike in search of gold, and finds a blizzard, a hungry partner and a dance-hall girl instead.' },
  { slug: 'safety-last', title: 'Safety Last!', year: 1923, genres: ['Comedy'], minutes: 70, cast: ['Harold Lloyd', 'Mildred Davis'], director: 'Fred C. Newmeyer, Sam Taylor', overview: 'A department-store clerk trying to impress his girlfriend ends up having to climb the outside of the store’s tall building himself.' },
  { slug: 'the-phantom-of-the-opera', title: 'The Phantom of the Opera', year: 1925, genres: ['Horror'], minutes: 93, cast: ['Lon Chaney', 'Mary Philbin', 'Norman Kerry'], director: 'Rupert Julian', overview: 'A masked figure haunting the Paris Opera House falls for a young singer and will stop at nothing to make her a star.' },
  { slug: 'the-cabinet-of-dr-caligari', title: 'The Cabinet of Dr. Caligari', year: 1920, genres: ['Horror'], minutes: 76, cast: ['Werner Krauss', 'Conrad Veidt', 'Lil Dagover'], director: 'Robert Wiene', overview: 'A travelling showman and the sleepwalker he exhibits arrive in a German town, and a string of murders begins.' },
  { slug: 'his-girl-friday', title: 'His Girl Friday', year: 1940, genres: ['Comedy'], minutes: 92, cast: ['Cary Grant', 'Rosalind Russell', 'Ralph Bellamy'], director: 'Howard Hawks', overview: 'A newspaper editor learns his ex-wife and best reporter is about to remarry, and lures her back for one last story.' },
  { slug: 'detour', title: 'Detour', year: 1945, genres: ['Crime', 'Film noir'], minutes: 68, cast: ['Tom Neal', 'Ann Savage'], director: 'Edgar G. Ulmer', overview: 'A nightclub pianist hitchhiking to Los Angeles takes a lift that goes very wrong, then picks up the wrong passenger.' },
  { slug: 'doa', title: 'D.O.A.', year: 1950, genres: ['Film noir'], minutes: 83, cast: ['Edmond O’Brien', 'Pamela Britton'], director: 'Rudolph Maté', overview: 'A man learns he has been poisoned and has only days to live, and spends them finding out who killed him.' },
  { slug: 'plan-9-from-outer-space', title: 'Plan 9 from Outer Space', year: 1957, genres: ['Science fiction', 'Horror'], minutes: 79, cast: ['Gregory Walcott', 'Mona McKinnon', 'Tor Johnson', 'Vampira', 'Bela Lugosi'], director: 'Ed Wood', overview: 'Aliens raise the dead to stop humanity from building a doomsday weapon.' },
  { slug: 'the-little-shop-of-horrors', title: 'The Little Shop of Horrors', year: 1960, genres: ['Comedy', 'Horror'], minutes: 72, cast: ['Jonathan Haze', 'Jackie Joseph', 'Mel Welles', 'Jack Nicholson'], director: 'Roger Corman', overview: 'A clumsy florist’s clerk raises a strange plant that brings in customers, and develops a taste for human blood.' },
  { slug: 'carnival-of-souls', title: 'Carnival of Souls', year: 1962, genres: ['Horror'], minutes: 78, cast: ['Candace Hilligoss', 'Sidney Berger'], director: 'Herk Harvey', overview: 'After surviving a car accident, a church organist starts a new job in another town and is drawn to an abandoned lakeside pavilion.' },
  { slug: 'charade', title: 'Charade', year: 1963, genres: ['Mystery', 'Romance'], minutes: 113, cast: ['Cary Grant', 'Audrey Hepburn', 'Walter Matthau'], director: 'Stanley Donen', overview: 'In Paris, a widow is pursued by several men who want the fortune her murdered husband stole, and she cannot tell which of them to trust.' },
  { slug: 'night-of-the-living-dead', title: 'Night of the Living Dead', year: 1968, genres: ['Horror'], minutes: 96, cast: ['Duane Jones', 'Judith O’Dea'], director: 'George A. Romero', overview: 'Strangers barricade themselves in a farmhouse in rural Pennsylvania as the dead rise and close in.', adultsOnly: true },
  { slug: 'elephants-dream', title: 'Elephants Dream', year: 2006, genres: ['Animation', 'Science fiction'], minutes: 11, cast: ['Tygo Gernandt', 'Cas Jansen'], director: 'Bassam Kurdali', overview: 'Two men explore a vast, strange machine that one of them knows well and the other does not understand at all. The first Blender open movie.' },
  { slug: 'big-buck-bunny', title: 'Big Buck Bunny', year: 2008, genres: ['Animation', 'Comedy'], minutes: 10, cast: [], director: 'Sacha Goedegebure', overview: 'A large, gentle rabbit is pushed too far by three bullying rodents and plans his revenge.' },
  { slug: 'sita-sings-the-blues', title: 'Sita Sings the Blues', year: 2008, genres: ['Animation', 'Musical'], minutes: 82, cast: ['Annette Hanshaw', 'Reena Shah', 'Aseem Chhabra'], director: 'Nina Paley', overview: 'The Ramayana, told through Sita’s eyes and the 1920s songs of Annette Hanshaw, alongside a modern story of a marriage falling apart.' },
  { slug: 'sintel', title: 'Sintel', year: 2010, genres: ['Animation', 'Fantasy'], minutes: 15, cast: ['Halina Reijn', 'Thom Hoffman'], director: 'Colin Levy', overview: 'A lonely young woman searches the world for the baby dragon she raised, who was taken from her.' },
  { slug: 'tears-of-steel', title: 'Tears of Steel', year: 2012, genres: ['Science fiction'], minutes: 12, cast: ['Derek de Lint', 'Sergio Hasselbaink', 'Vanja Rukavina', 'Denise Rebergen', 'Rogier Schippers'], director: 'Ian Hubert', overview: 'In a future Amsterdam, a group of scientists tries to save the world from robots by restaging a breakup.' },
  { slug: 'cosmos-laundromat', title: 'Cosmos Laundromat', year: 2015, genres: ['Animation', 'Comedy'], minutes: 12, cast: ['Pierre Bokma', 'Reinout Scholten van Aschat'], director: 'Mathieu Auvray', overview: 'On a desolate island, a suicidal sheep meets a mysterious salesman who offers him the gift of a lifetime.' },
  { slug: 'spring', title: 'Spring', year: 2019, genres: ['Animation', 'Fantasy'], minutes: 8, cast: [], director: 'Andy Goralczyk', overview: 'A shepherd girl and her dog face ancient spirits in order to keep the cycle of life going.' },
];
// Episodes: [number, name, year first shown]. Gaps are episodes this library does not have.
const DEMO_SHOWS = [
  {
    show: 'Superman', slug: 'superman', year: 1941, minutes: 9, genres: ['Animation', 'Action'],
    overview: 'Theatrical cartoons from Fleischer and then Famous Studios in which Superman, as Clark Kent of the Daily Planet, takes on mad scientists, robots, monsters and saboteurs.',
    cast: ['Bud Collyer as Superman / Clark Kent', 'Joan Alexander as Lois Lane', 'Jackson Beck as the narrator'],
    seasons: [
      [[1, 'Superman', 1941], [2, 'The Mechanical Monsters', 1941], [3, 'Billion Dollar Limited', 1942], [4, 'The Arctic Giant', 1942], [5, 'The Bulleteers', 1942], [6, 'The Magnetic Telescope', 1942], [7, 'Electric Earthquake', 1942], [8, 'Volcano', 1942], [9, 'Terror on the Midway', 1942]],
      [[2, 'Showdown', 1942], [4, 'Destruction, Inc.', 1942], [5, 'The Mummy Strikes', 1943], [7, 'The Underground World', 1943], [8, 'Secret Agent', 1943]],
    ],
  },
  {
    show: 'Popeye the Sailor', slug: 'popeye-the-sailor', year: 1936, minutes: 17, genres: ['Animation', 'Comedy'],
    overview: 'The Fleischer two-reel Technicolor specials, in which Popeye takes on Sindbad the Sailor, and then Abu Hassan and his forty thieves.',
    cast: ['Jack Mercer as Popeye', 'Mae Questel as Olive Oyl', 'Gus Wickie as Sindbad / Abu Hassan'],
    seasons: [[[1, 'Popeye the Sailor Meets Sindbad the Sailor', 1936], [2, 'Popeye the Sailor Meets Ali Baba’s Forty Thieves', 1937]]],
  },
  {
    show: 'The Beverly Hillbillies', slug: 'the-beverly-hillbillies', year: 1962, minutes: 26, genres: ['Comedy'],
    overview: 'After oil is found on his land, mountaineer Jed Clampett moves his family to a mansion in Beverly Hills, next door to the banker who looks after their money.',
    cast: ['Buddy Ebsen as Jed Clampett', 'Irene Ryan as Granny', 'Donna Douglas as Elly May Clampett', 'Max Baer Jr. as Jethro Bodine', 'Raymond Bailey as Milburn Drysdale'],
    seasons: [
      [[2, 'Getting Settled', 1962], [7, 'The Servants', 1962], [10, 'Pygmalion and Elly', 1962], [17, 'Jed’s Dilemma', 1963], [18, 'Jed Saves the Drysdales’ Marriage', 1963], [25, 'The Family Tree', 1963]],
      [[3, 'Granny’s Garden', 1963], [5, 'The Clampett Look', 1963], [7, 'Chickadee Returns', 1963]],
    ],
  },
];
// Tracks: [title, seconds].
const DEMO_ALBUMS = [
  { slug: 'the-animal-economy', artist: 'MonoCulture', album: 'The Animal Economy', year: 2006, genre: 'IDM', tracks: [['Peace City', 159], ['The New Rasputin', 165], ['My Cancer for a Kingdom', 228], ['Parco Girls', 257], ['Cause and Consequence', 319], ['Syntig (Shuji Monomix)', 247], ['Tone Service (Mi Casa Su Casa Monomix)', 248], ['Haragei (with Shudo Horns)', 187], ['Saori', 161]] },
  { slug: 'the-four-seasons', artist: 'Memphis', album: 'The Four Seasons', year: 2011, genre: 'Ambient', tracks: [['Spring I', 277], ['Spring II', 228], ['Spring III', 420], ['Summer I', 228], ['Summer II', 238], ['Summer III', 477], ['Autumn I', 305], ['Autumn II', 262], ['Autumn III', 378], ['Winter I', 165], ['Winter II (Safe Passage)', 231], ['Winter III', 546]] },
  { slug: 'experiments-at-the-edge-of-time', artist: 'Softspace', album: 'Experiments at the Edge of Time', year: 2006, genre: 'Ambient', tracks: [['Candle Flicker', 345], ['Subliminal', 429], ['Loons', 523], ['Observatory One', 512], ['Sun', 169], ['Smiling Eyes', 176], ['Summer Longings', 157], ['Contemplation', 218], ['Improvised One', 220], ['Off Her Lips', 299], ['Shadowed Forest', 327]] },
  { slug: 'paramo', artist: 'Callejas', album: 'Páramo', year: 2021, genre: 'Dub techno', tracks: [['Páramo 1', 325], ['Páramo 2', 302], ['Páramo 2 (Brendon Moeller Dub Remix)', 384], ['Páramo 2 (Magdalena Remix)', 403], ['Páramo 1 (Archaic Revival Remix)', 483], ['Páramo 1 (Eviru Dibad Remix)', 383]] },
];
// [folder, hosted folder, photos in it]
const DEMO_PHOTO_ALBUMS = [['Beach 2026', 'beach-2026', 12], ['Birthday', 'birthday', 8]];
const DEMO_VIDEOS = ['First steps', 'Road trip day 1', 'Road trip day 2', 'School play', 'Snow day'];
const DEMO_PLAY_NOTE = 'Sample library: there is no file to play';
const POSTER_LINE_CHARS = 11;

/** A placeholder poster, while a hosted one loads or when it cannot: the title large and wrapped, over a gradient. */
function posterSvg(title, kicker, [c1, c2], shape) {
  const w = 400;
  const h = shape === 'square' ? 400 : 600;
  const lines = [];
  for (const word of title.toUpperCase().split(' ')) {
    const last = lines[lines.length - 1];
    if (last && (last + ' ' + word).length <= POSTER_LINE_CHARS) lines[lines.length - 1] = `${last} ${word}`;
    else lines.push(word);
  }
  const size = lines.some((l) => l.length > 9) ? 40 : 50;
  const top = h * 0.62 - (lines.length - 1) * size * 0.55;
  const text = lines.map((line, i) => `<text x="30" y="${top + i * size * 1.1}" font-family="Arial Black, Arial, sans-serif" font-weight="900" font-size="${size}" fill="#fff">${line}</text>`).join('');
  return (
    'data:image/svg+xml;utf8,' +
    encodeURIComponent(
      `<svg xmlns="http://www.w3.org/2000/svg" width="${w}" height="${h}">` +
        `<defs><linearGradient id="g" x1="0" y1="0" x2="0.4" y2="1"><stop offset="0" stop-color="${c1}"/><stop offset="1" stop-color="${c2}"/></linearGradient></defs>` +
        `<rect width="${w}" height="${h}" fill="url(#g)"/><circle cx="${w * 0.78}" cy="${h * 0.22}" r="${w * 0.42}" fill="#fff" opacity=".06"/>` +
        `<text x="30" y="${top - size * 1.1}" font-family="monospace" font-size="15" letter-spacing="3" fill="rgba(255,255,255,.7)">${kicker}</text>${text}</svg>`
    )
  );
}

export class DemoTheater {
  isDemo = true;

  constructor() {
    const now = Date.now();
    this.libraries = [
      { id: 'films', name: 'Films', kind: 'movies', path: 'D:\\Media\\Films', audience: 'everyone', added: now - 90 * DAY, scanned: now - HOUR, count: 0 },
      { id: 'shows', name: 'Shows', kind: 'shows', path: 'D:\\Media\\Shows', audience: 'family', added: now - 90 * DAY, scanned: now - HOUR, count: 0 },
      { id: 'music', name: 'Music', kind: 'music', path: 'D:\\Media\\Music', audience: 'everyone', added: now - 90 * DAY, scanned: now - HOUR, count: 0 },
      { id: 'photos', name: 'Photos', kind: 'photos', path: 'C:\\Users\\alex\\Pictures', audience: 'family', added: now - 30 * DAY, scanned: now - HOUR, count: 0 },
      { id: 'home', name: 'Home videos', kind: 'videos', path: 'D:\\Media\\Home videos', audience: 'admins', added: now - 30 * DAY, scanned: now - HOUR, count: 0 },
    ];
    const base = { show: '', season: 0, episode: 0, artist: '', album: '', track: 0, genre: '', folder: '', size: 0, cover: false, subtitles: [], tags: [], adultsOnly: false, year: 0 };
    const items = [];
    DEMO_FILMS.forEach((f, i) => items.push({ ...base, id: `film-${f.slug}`, library: 'films', kind: 'movie', title: f.title, year: f.year, path: `D:\\Media\\Films\\${f.title} (${f.year}).mkv`, size: Math.round((0.6 + f.minutes / 60) * GB), modified: now - (i + 1) * 4 * DAY, added: now - (i + 1) * 4 * DAY, adultsOnly: !!f.adultsOnly, subtitles: i % 2 ? [] : [{ label: 'en', file: '' }] }));
    DEMO_SHOWS.forEach((s) =>
      s.seasons.forEach((episodes, season) =>
        episodes.forEach(([e, name, year], n) => {
          const code = `S${String(season + 1).padStart(2, '0')}E${String(e).padStart(2, '0')}`;
          items.push({ ...base, id: `ep-${s.slug}-${season + 1}-${e}`, library: 'shows', kind: 'episode', title: name, show: s.show, season: season + 1, episode: e, year, path: `D:\\Media\\Shows\\${s.show}\\Season ${season + 1}\\${s.show} - ${code}.mp4`, size: s.minutes * 22 * MB, modified: now - (season * 10 + n + 1) * DAY, added: now - (season * 10 + n + 1) * DAY });
        })
      )
    );
    DEMO_ALBUMS.forEach((a) =>
      a.tracks.forEach(([title, seconds], t) => items.push({ ...base, id: `track-${a.slug}-${t + 1}`, library: 'music', kind: 'track', title, artist: a.artist, album: a.album, track: t + 1, year: a.year, genre: a.genre, folder: a.album, path: `D:\\Media\\Music\\${a.artist}\\${a.album}\\${String(t + 1).padStart(2, '0')} ${title}.flac`, size: Math.round(seconds * 0.11 * MB), modified: now - 40 * DAY, added: now - 40 * DAY }))
    );
    DEMO_PHOTO_ALBUMS.forEach(([folder, , count], pi) => {
      for (let n = 1; n <= count; n++) items.push({ ...base, id: `photo-${pi}-${n}`, library: 'photos', kind: 'photo', title: `IMG ${String(n).padStart(4, '0')}`, folder, path: `C:\\Users\\alex\\Pictures\\${folder}\\IMG_${n}.jpg`, size: 4 * MB, modified: now - (pi * 20 + n) * HOUR, added: now - (pi * 20 + n) * HOUR });
    });
    DEMO_VIDEOS.forEach((title, i) => items.push({ ...base, id: `video-${i}`, library: 'home', kind: 'video', title, folder: '2026', path: `D:\\Media\\Home videos\\2026\\${title}.mp4`, size: 1.2 * GB, modified: now - (i + 2) * DAY, added: now - (i + 2) * DAY }));
    this.items = items;
    for (const library of this.libraries) library.count = items.filter((i) => i.library === library.id).length;
  }

  async theater() {
    return { libraries: structuredClone(this.libraries), scanning: false, ffmpeg: true, suggestions: [{ path: 'C:\\Users\\alex\\Videos', kind: 'videos', name: 'Videos' }] };
  }

  async theaterItems(library = null) {
    return structuredClone(library ? this.items.filter((i) => i.library === library) : this.items);
  }

  #tint(id) {
    let n = 0;
    for (const c of id) n = (n * 31 + c.charCodeAt(0)) % DEMO_TINTS.length;
    return DEMO_TINTS[n];
  }

  async theaterArt(id, kind = 'thumb') {
    const item = this.items.find((i) => i.id === id);
    if (!item) return null;
    if (kind === 'poster') {
      if (item.kind === 'photo' || item.kind === 'video') return null;
      const title = item.show || item.album || item.title;
      const kicker = item.kind === 'track' ? item.artist.toUpperCase() : item.kind === 'episode' ? 'SERIES' : String(item.year);
      const placeholder = posterSvg(title, kicker, this.#tint(title), item.kind === 'track' ? 'square' : 'tall');
      if (item.kind === 'movie') return hosted(`theater/posters/${DEMO_FILMS.find((f) => f.title === item.title).slug}.jpg`, placeholder);
      if (item.kind === 'episode') return hosted(`theater/posters/${DEMO_SHOWS.find((s) => s.show === item.show).slug}.jpg`, placeholder);
      return hosted(`theater/covers/${DEMO_ALBUMS.find((a) => a.album === item.album).slug}.jpg`, placeholder);
    }
    const [a, b] = this.#tint(item.id);
    const placeholder = svgThumb(item.title.toUpperCase(), a, b);
    if (item.kind === 'episode') return hosted(`theater/stills/${DEMO_SHOWS.find((s) => s.show === item.show).slug}-s${item.season}e${item.episode}.jpg`, placeholder);
    if (item.kind === 'photo') {
      const [, folder] = DEMO_PHOTO_ALBUMS.find(([name]) => name === item.folder);
      return hosted(`theater/photos/${folder}/${item.title.slice(-2)}.jpg`, placeholder);
    }
    return placeholder;
  }

  async theaterDetails(id) {
    const item = this.items.find((i) => i.id === id);
    if (!item) throw new Error('That is no longer in Theater');
    const probe = item.kind === 'photo' ? null : { seconds: item.kind === 'video' ? 95 : 0, width: 1920, height: 1080, video: item.kind === 'track' ? '' : 'h264', audio: item.kind === 'track' ? 'flac' : 'aac', bitrate: 8e6 };
    if (item.kind === 'movie') {
      const f = DEMO_FILMS.find((x) => x.title === item.title);
      probe.seconds = f.minutes * 60;
      return { details: { title: f.title, year: f.year, overview: f.overview, cast: f.cast, director: f.director, genres: f.genres, runtimeMinutes: f.minutes, url: '', source: 'Sample data', posterUrl: null }, episode: null, probe, poster: true };
    }
    if (item.kind === 'episode') {
      const s = DEMO_SHOWS.find((x) => x.show === item.show);
      probe.seconds = s.minutes * 60;
      return { details: { title: s.show, year: s.year, overview: s.overview, cast: s.cast, director: '', genres: s.genres, runtimeMinutes: s.minutes, url: '', source: 'Sample data' }, episode: { name: item.title, overview: '', aired: String(item.year), runtimeMinutes: s.minutes }, probe, poster: true };
    }
    if (item.kind === 'track') {
      const a = DEMO_ALBUMS.find((x) => x.album === item.album);
      probe.seconds = a.tracks[item.track - 1][1];
      return { details: { title: item.album, year: item.year, overview: '', cast: [], director: item.artist, genres: [item.genre], runtimeMinutes: Math.round(a.tracks.reduce((sum, [, sec]) => sum + sec, 0) / 60), url: '', source: 'Sample data' }, episode: null, probe, poster: true };
    }
    return { details: null, episode: null, probe, poster: false };
  }

  async theaterPlay() {
    throw new Error(DEMO_PLAY_NOTE);
  }

  async theaterEdit(changes) {
    const item = this.items.find((i) => i.id === changes.id);
    if (!item) throw new Error('That is no longer in Theater');
    if (changes.tags) item.tags = [...new Set(changes.tags.map((t) => t.trim()).filter(Boolean))];
    if (changes.adultsOnly != null) item.adultsOnly = changes.adultsOnly;
    if (changes.title) item.title = changes.title;
    if (changes.year != null) item.year = changes.year;
    return structuredClone(item);
  }

  async theaterScan() {
    return { started: true };
  }

  async theaterLibraryAdd(library) {
    const made = { id: `lib-${this.libraries.length}`, name: library.name || library.path.split('\\').pop(), kind: library.kind, path: library.path, audience: library.audience || 'everyone', added: Date.now(), scanned: Date.now(), count: 0 };
    this.libraries.push(made);
    return structuredClone(made);
  }

  async theaterLibraryUpdate(changes) {
    const library = this.libraries.find((l) => l.id === changes.id);
    if (changes.audience) library.audience = changes.audience;
    if (changes.name) library.name = changes.name;
    return structuredClone(library);
  }

  async theaterLibraryRemove(id) {
    this.libraries = this.libraries.filter((l) => l.id !== id);
    this.items = this.items.filter((i) => i.library !== id);
  }

  async theaterSubtitles() {
    throw new Error('Sample library: add an OpenSubtitles key on a real box to fetch subtitles');
  }

  async theaterFolders(path) {
    const tree = { '': ['C:\\', 'D:\\'], 'C:\\': ['Users'], 'C:\\Users': ['alex'], 'C:\\Users\\alex': ['Videos', 'Pictures', 'Music'], 'D:\\': ['Media'], 'D:\\Media': ['Films', 'Shows', 'Music', 'Home videos'] };
    return (tree[path] || []).map((name) => ({ path: path ? (path.endsWith('\\') ? path + name : `${path}\\${name}`) : name, name: name.replace(/\\$/, '') }));
  }
}
