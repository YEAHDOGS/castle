// Sample data for everything outside Storage on /demo: people, AI models, the arcade
// library and clones. Like demo.js, it is the same on every visit and nothing is saved.
// Real boxes replace each of these with a call to the box; where a section has no box
// call yet, the section says it is showing sample data.

import { svgThumb } from './adapter.js';

const GB = 1024 ** 3;
const MB = 1024 ** 2;
const MINUTE = 60000;
const HOUR = 60 * MINUTE;
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

/** A pretend Penguin automation that walks through a launch, then counts coins. */
export class DemoClones {
  isDemo = true;

  constructor() {
    this.state = 'stopped';
    this.username = '';
    this.hasPassword = false;
    this.log = [];
    this.startedAt = 0;
  }

  /** Advances the pretend launch by wall-clock time, so polling shows progress. */
  tick() {
    if (this.state !== 'launching' && this.state !== 'running') return;
    const elapsed = Date.now() - this.startedAt;
    const shown = Math.min(DEMO_LAUNCH_LOG.length, Math.floor(elapsed / LAUNCH_STEP_MS) + 1);
    this.log = DEMO_LAUNCH_LOG.slice(0, shown);
    if (shown < DEMO_LAUNCH_LOG.length) return;
    this.state = 'running';
    const runs = Math.floor((elapsed - DEMO_LAUNCH_LOG.length * LAUNCH_STEP_MS) / COIN_TICK_MS);
    for (let run = 1; run <= Math.min(runs, 6); run++) {
      this.log.push(`cart game ${run} finished: +${COINS_PER_RUN} coins (${run * COINS_PER_RUN} this session)`);
    }
  }

  status() {
    this.tick();
    return {
      id: 'penguin',
      name: 'Penguin AutoSurf',
      description: 'Signs in to Club Penguin Journey and plays Cart Surfer on a loop for coins.',
      target: 'debian-x11',
      state: this.state,
      repoPresent: true,
      username: this.username,
      hasPassword: this.hasPassword,
      log: this.log,
    };
  }

  async clones() {
    return [this.status()];
  }

  async cloneLogin(_id, username, password) {
    if (!username.trim()) throw new Error('that name or password cannot be saved');
    this.username = username.trim();
    if (password) this.hasPassword = true;
    return this.status();
  }

  async cloneStart() {
    if (!this.username || !this.hasPassword) throw new Error('save the account name and password first');
    if (this.state === 'launching' || this.state === 'running') throw new Error('it is already running');
    this.state = 'launching';
    this.startedAt = Date.now();
    return this.status();
  }

  async cloneStop() {
    this.state = 'stopped';
    this.log = [...this.log, '  [STOP] Sent ACPI power-off -- the guest is shutting down.'];
    return this.status();
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
export class DemoBox {
  isDemo = true;

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
