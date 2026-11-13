// Sample data for everything outside Storage on /demo: people, AI models, the arcade
// library and clones. Like demo.js, it is the same on every visit and nothing is saved.
// Real boxes replace each of these with a call to the box; where a section has no box
// call yet, the section says it is showing sample data.

import { svgThumb } from './adapter.js';

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

const TITLE_FIRST = ['Sky', 'Iron', 'Pixel', 'Turbo', 'Moon', 'Crystal', 'Shadow', 'Rocket', 'Thunder', 'Neon', 'Star', 'Jungle'];
const TITLE_SECOND = ['Runner', 'Quest', 'Racers', 'Knights', 'Kart', 'Legends', 'Brawl', 'Odyssey', 'Tactics', 'Harvest', 'Circuit', 'Dungeon'];
const SYSTEM_TINTS = [
  ['#4a1a1a', '#0e0508'], ['#3a1a4a', '#0b0e1e'], ['#1a4a2b', '#0e1e12'], ['#2b3a8a', '#0e122b'],
  ['#8a5a2b', '#2b1a0e'], ['#123a4a', '#0e0b1e'],
];
const DEMO_SYSTEM_SPECS = [
  { short: 'NES', name: 'Nintendo - Nintendo Entertainment System', emulator: 'RetroArch', games: 9, each: 0.3 * MB },
  { short: 'SNES', name: 'Nintendo - Super Nintendo Entertainment System', emulator: 'RetroArch', games: 24, each: 2 * MB },
  { short: 'N64', name: 'Nintendo - Nintendo 64', emulator: 'RetroArch', games: 18, each: 19 * MB },
  { short: 'GameCube', name: 'Nintendo - GameCube', emulator: 'RetroArch', games: 14, each: 760 * MB },
  { short: 'GBA', name: 'Nintendo - Game Boy Advance', emulator: 'RetroArch', games: 12, each: 12 * MB },
  { short: 'PS1', name: 'Sony - PlayStation', emulator: 'RetroArch', games: 7, each: 520 * MB, discs: true },
  { short: 'PS2', name: 'Sony - PlayStation 2', emulator: 'PCSX2', games: 8, each: 2.7 * GB, noArt: true, discs: true },
  { short: 'PSP', name: 'Sony - PlayStation Portable', emulator: 'PPSSPP', games: 6, each: 1.2 * GB, noArt: true },
];

function demoGames(spec, systemIndex) {
  return Array.from({ length: spec.games }, (_, i) => {
    const title = `${TITLE_FIRST[(i * 5 + systemIndex * 3) % TITLE_FIRST.length]} ${TITLE_SECOND[(i * 7 + systemIndex) % TITLE_SECOND.length]}${i >= TITLE_SECOND.length ? ` ${Math.floor(i / TITLE_SECOND.length) + 1}` : ''} (USA)`;
    const tint = SYSTEM_TINTS[(i + systemIndex) % SYSTEM_TINTS.length];
    const size = Math.round(spec.each * (0.7 + ((i * 37) % 60) / 100));
    // On disc-based systems every fifth game comes on several discs, shown as one game.
    const discCount = spec.discs && i % 5 === 1 ? 2 + (i % 3) : 1;
    return {
      id: i,
      title,
      size: size * discCount,
      present: true,
      hasBoxart: !spec.noArt && i % 9 !== 8,
      hasSnap: !spec.noArt,
      discs: Array.from({ length: discCount }, (_, d) => ({ label: discCount > 1 ? `Disc ${d + 1}` : '', size, present: true })),
      art: svgThumb(spec.short, tint[0], tint[1]),
      snap: svgThumb(`${spec.short} · IN GAME`, tint[1], tint[0]),
    };
  });
}

const DEMO_GENRES = ['platformer', 'role-playing game', 'racing game', 'puzzle game', 'action-adventure', 'fighting game'];

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
    if (kind === 'snap') return found?.hasSnap ? found.snap : null;
    return found?.hasBoxart ? found.art : null;
  }

  async arcadePlay(system, game, disc = 0) {
    const found = (await this.arcadeGames(system))[game];
    const label = found?.discs?.[disc]?.label;
    return { started: `${found?.title || ''}${label ? ` (${label})` : ''}`, demo: true };
  }

  /** A made-up description, so the demo's cards read like a real box's. */
  async arcadeAbout(system, game) {
    const spec = this.library.find((s) => s.spec.name === system)?.spec;
    const found = (await this.arcadeGames(system))[game];
    if (!found || !spec) return null;
    const name = found.title.replace(/ \(.*$/, '');
    const genre = DEMO_GENRES[(game + spec.short.length) % DEMO_GENRES.length];
    return {
      title: name,
      extract: `${name} is a sample ${genre} for the ${spec.short}. This description is sample text: on a real box, Castle asks Wikipedia about a game the first time its card is opened and keeps the answer.`,
      url: 'https://en.wikipedia.org/wiki/Video_game',
    };
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
    return { targets: DEMO_TARGETS, machines: this.machines, bench: this.bench };
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
// Theater: a made-up library of films, shows, albums, photos and home videos. Every title,
// name and summary here is invented sample data.
// ---------------------------------------------------------------------------------------

const DEMO_TINTS = [
  ['#5a1a2b', '#0e0508'], ['#1a3a5a', '#05080e'], ['#2b5a1a', '#080e05'], ['#5a4a1a', '#0e0b05'],
  ['#3a1a5a', '#08050e'], ['#1a5a52', '#050e0d'], ['#5a2b1a', '#0e0805'], ['#24245a', '#05050e'],
];
const DEMO_FILMS = [
  { title: 'Night Harbor', year: 2019, genres: ['Thriller', 'Mystery'], minutes: 118, cast: ['Ada Brennan', 'Tomas Ruiz', 'Lena Okafor'], director: 'Mira Holt', overview: 'A harbour pilot finds a ship that should not exist and a crew that will not say where it came from.' },
  { title: 'The Glass Orchard', year: 2021, genres: ['Drama'], minutes: 104, cast: ['Ruth Calder', 'Ivo Marsh'], director: 'Sanne Vos', overview: 'Three sisters return to the family greenhouse the summer it is sold.' },
  { title: 'Signal Lost', year: 2016, genres: ['Science fiction'], minutes: 131, cast: ['Kai Moreno', 'Hana Ito', 'Peter Lund'], director: 'Oren Bakr', overview: 'A relay station at the edge of the solar system goes quiet. The repair crew finds out why.' },
  { title: 'Dust and Neon', year: 2023, genres: ['Action', 'Crime'], minutes: 97, cast: ['Jo Varga', 'Nils Achebe'], director: 'Tess Arden', overview: 'A courier with one night left on her licence takes the job everyone else refused.' },
  { title: 'Paper Moons', year: 2012, genres: ['Animation', 'Family'], minutes: 88, cast: ['Ellie Park', 'Ben Szabo'], director: 'Uma Reyes', overview: 'A girl who folds paper animals wakes up to find them gone, and a trail of paper footprints.' },
  { title: 'Northbound', year: 2018, genres: ['Horror'], minutes: 109, cast: ['Greta Lind', 'Sol Mensah'], director: 'Arne Falk', overview: 'The last train of the winter leaves the station with one more passenger than tickets.', adultsOnly: true },
];
const DEMO_SHOWS = [
  { show: 'Kitchen Wars', seasons: [4, 3], genres: ['Reality', 'Food'], overview: 'Twelve home cooks, one kitchen, and a judge who has never said "good".', cast: ['Rosa Quint as Host', 'Dev Malik as Judge'] },
  { show: 'The Long Road', seasons: [6], genres: ['Drama', 'Western'], overview: 'A family drives cattle across a dry country in the last year anyone will let them.', cast: ['Hal Brody as Amos', 'June Tao as Clara'] },
];
const DEMO_ALBUMS = [
  { artist: 'Low Hum', album: 'Tidal', year: 2024, genre: 'Ambient', tracks: ['Shoreline', 'Undertow', 'Salt', 'Slack Water', 'Riptide', 'Low Tide', 'Pier', 'Lanterns'] },
  { artist: 'Marrow', album: 'Signals', year: 2020, genre: 'Electronic', tracks: ['Carrier', 'Static Bloom', 'Handshake', 'Long Wave', 'Echo Room', 'Sign Off'] },
];
const DEMO_PHOTO_ALBUMS = [['Beach 2026', 12], ['Birthday', 8]];
const DEMO_VIDEOS = ['First steps', 'Road trip day 1', 'Road trip day 2', 'School play', 'Snow day'];
const DEMO_PLAY_NOTE = 'Sample library: there is no file to play';
const POSTER_LINE_CHARS = 11;

/** A made-up poster: the title large and wrapped, over a gradient. Tall for films and shows, square for albums. */
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
    DEMO_FILMS.forEach((f, i) => items.push({ ...base, id: `film-${i}`, library: 'films', kind: 'movie', title: f.title, year: f.year, path: `D:\\Media\\Films\\${f.title} (${f.year}).mkv`, size: (4 + i) * GB, modified: now - (i + 1) * 9 * DAY, added: now - (i + 1) * 9 * DAY, adultsOnly: !!f.adultsOnly, subtitles: i % 2 ? [] : [{ label: 'en', file: '' }] }));
    DEMO_SHOWS.forEach((s, si) =>
      s.seasons.forEach((count, season) => {
        for (let e = 1; e <= count; e++) {
          items.push({ ...base, id: `ep-${si}-${season + 1}-${e}`, library: 'shows', kind: 'episode', title: `Episode ${e}`, show: s.show, season: season + 1, episode: e, path: `D:\\Media\\Shows\\${s.show}\\Season ${season + 1}\\E${e}.mp4`, size: 900 * MB, modified: now - (season * 10 + e) * DAY, added: now - (season * 10 + e) * DAY });
        }
      })
    );
    DEMO_ALBUMS.forEach((a, ai) =>
      a.tracks.forEach((title, t) => items.push({ ...base, id: `track-${ai}-${t}`, library: 'music', kind: 'track', title, artist: a.artist, album: a.album, track: t + 1, year: a.year, genre: a.genre, folder: a.album, path: `D:\\Media\\Music\\${a.artist}\\${a.album}\\${t + 1} ${title}.flac`, size: 28 * MB, modified: now - 40 * DAY, added: now - 40 * DAY }))
    );
    DEMO_PHOTO_ALBUMS.forEach(([folder, count], pi) => {
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
      const kicker = item.kind === 'track' ? item.artist.toUpperCase() : item.kind === 'episode' ? 'A SAMPLE SERIES' : String(item.year);
      return posterSvg(title, kicker, this.#tint(title), item.kind === 'track' ? 'square' : 'tall');
    }
    const [a, b] = this.#tint(item.id);
    return svgThumb(item.title.toUpperCase(), a, b);
  }

  async theaterDetails(id) {
    const item = this.items.find((i) => i.id === id);
    if (!item) throw new Error('That is no longer in Theater');
    const probe = item.kind === 'photo' ? null : { seconds: item.kind === 'track' ? 214 + item.track * 7 : item.kind === 'episode' ? 2640 : item.kind === 'video' ? 95 : 0, width: 1920, height: 1080, video: item.kind === 'track' ? '' : 'h264', audio: item.kind === 'track' ? 'flac' : 'aac', bitrate: 8e6 };
    if (item.kind === 'movie') {
      const f = DEMO_FILMS.find((x) => x.title === item.title);
      probe.seconds = f.minutes * 60;
      return { details: { title: f.title, year: f.year, overview: f.overview, cast: f.cast, director: f.director, genres: f.genres, runtimeMinutes: f.minutes, url: '', source: 'Sample data', posterUrl: null }, episode: null, probe, poster: true };
    }
    if (item.kind === 'episode') {
      const s = DEMO_SHOWS.find((x) => x.show === item.show);
      return { details: { title: s.show, year: 2022, overview: s.overview, cast: s.cast, director: '', genres: s.genres, runtimeMinutes: 44, url: '', source: 'Sample data' }, episode: { name: `${item.title} of season ${item.season}`, overview: 'An invented episode summary for the sample library.', aired: '2022-03-01', runtimeMinutes: 44 }, probe, poster: true };
    }
    if (item.kind === 'track') {
      return { details: { title: item.album, year: item.year, overview: '', cast: [], director: item.artist, genres: [item.genre], runtimeMinutes: 0, url: '', source: 'Sample data' }, episode: null, probe, poster: true };
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
