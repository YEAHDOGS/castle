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
