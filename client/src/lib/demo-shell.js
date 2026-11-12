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
  { short: 'PS1', name: 'Sony - PlayStation', emulator: 'RetroArch', games: 7, each: 520 * MB },
  { short: 'PS2', name: 'Sony - PlayStation 2', emulator: 'PCSX2', games: 8, each: 2.7 * GB, noArt: true },
  { short: 'PSP', name: 'Sony - PlayStation Portable', emulator: 'PPSSPP', games: 6, each: 1.2 * GB, noArt: true },
];

function demoGames(spec, systemIndex) {
  return Array.from({ length: spec.games }, (_, i) => {
    const title = `${TITLE_FIRST[(i * 5 + systemIndex * 3) % TITLE_FIRST.length]} ${TITLE_SECOND[(i * 7 + systemIndex) % TITLE_SECOND.length]}${i >= TITLE_SECOND.length ? ` ${Math.floor(i / TITLE_SECOND.length) + 1}` : ''} (USA)`;
    const tint = SYSTEM_TINTS[(i + systemIndex) % SYSTEM_TINTS.length];
    return {
      id: i,
      title,
      size: Math.round(spec.each * (0.7 + ((i * 37) % 60) / 100)),
      present: true,
      hasBoxart: !spec.noArt && i % 9 !== 8,
      hasSnap: !spec.noArt,
      art: svgThumb(spec.short, tint[0], tint[1]),
    };
  });
}

/** The demo arcade, answering the same calls a box does. */
const DEMO_SAVED_GAMES = 3; // games per system that have sample saves
const DEMO_MAX_VERSIONS = 4;

export class DemoArcade {
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

  async arcadeArt(system, game) {
    const found = (await this.arcadeGames(system))[game];
    return found?.hasBoxart ? found.art : null;
  }

  async arcadePlay(system, game) {
    const found = (await this.arcadeGames(system))[game];
    return { started: found?.title || '', demo: true };
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
  { id: 'endeavouros', name: 'EndeavourOS Linux', description: 'Friendly Arch-based distribution.', family: 'linux', runtime: '', cloud: false, inject: true, built: true, disks: 1 },
  { id: 'cachyos', name: 'CachyOS Linux', description: 'Performance-tuned Arch-based distribution.', family: 'linux', runtime: '', cloud: false, inject: false, built: false, disks: 0 },
  { id: 'ubuntu-cloud', name: 'Ubuntu 24.04 LTS (headless)', description: 'SSH-ready in a minute.', family: 'linux', runtime: '', cloud: true, inject: false, built: false, disks: 0 },
  { id: 'win11-pro', name: 'Windows 11 Pro', description: 'Unattended install.', family: 'windows', runtime: '', cloud: false, inject: false, built: true, disks: 1 },
  { id: 'android-avd', name: 'Android 15 (Google emulator)', description: 'Play Store image. Installs the APKs you give it.', family: 'android', runtime: 'avd', apk: true, cloud: false, inject: false, built: false, disks: 0 },
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
    return { id: bot.id, name: bot.name, key: `a:${a.id}:${bot.id}`, state, ready: values.every((v) => v.from !== 'missing'), ownDisk: index > 0 || a.temporary, values, log };
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
