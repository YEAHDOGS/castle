// Box connection: the adapter that talks to a real Castle box (the castled daemon) over
// its HTTP API, plus where the connection is remembered.
//
// Every client -- web, desktop, Android, iOS -- uses this same adapter. Nothing here needs
// Samba or any other file-sharing protocol on the device.

import { StorageAdapter, LocalStorageAdapter } from './adapter.js';
import { isDemoRoute, demoNodes } from './demo.js';

const BOX_KEY = 'castle-box-v1';
const API_BASE = '/api/v1';
const ROOT_ID = 'root'; // what the UI calls the top folder; the box calls it ''
const CONNECT_HASH = '#connect=';
export const DEFAULT_BOX_URL = 'http://127.0.0.1:8787';
const THUMB_MAX_BYTES = 3 * 1024 * 1024; // images up to this size get a grid thumbnail
const TEXT_MAX_BYTES = 20000; // text files up to this size get an inline preview
const TEXT_MIMES = ['application/json'];
const UNREACHABLE = "Can't reach the box";
const LOCAL_HOSTS = ['127.0.0.1', 'localhost']; // a connect link may only name a box on this computer

/** @returns {{ url: string, token: string } | null} the remembered box, if any */
export function readBox() {
  try {
    const box = JSON.parse(localStorage.getItem(BOX_KEY));
    return box?.url && box?.token ? box : null;
  } catch {
    return null;
  }
}

function writeBox(box) {
  try {
    localStorage.setItem(BOX_KEY, JSON.stringify(box));
  } catch {
    /* storage blocked: the connection lasts for this session only */
  }
}

export function forgetBox() {
  try {
    localStorage.removeItem(BOX_KEY);
  } catch {
    /* nothing to forget */
  }
}

/**
 * When the box serves this app it prints a link ending in #connect=<token>. Opening that
 * link connects in one step: take the token, then wipe it from the address bar and history.
 * @returns {{ url: string, token: string } | null}
 */
export function takeConnectLink() {
  if (typeof location === 'undefined' || !location.hash.startsWith(CONNECT_HASH)) return null;
  if (isDemoRoute()) return null; // the demo never talks to a box
  // "#connect=<token>" means the box serving this page. "#connect=<token>&box=<address>"
  // names another one, which is how the live development interface reaches the box.
  const [token, ...rest] = location.hash.slice(CONNECT_HASH.length).split('&');
  const named = new URLSearchParams(rest.join('&')).get('box');
  history.replaceState(null, '', location.pathname + location.search);
  if (!token) return null;
  if (!named) return { url: location.origin, token };
  // Only a box on this computer: a link must not be able to send your files to someone else's box.
  try {
    const url = new URL(named);
    return LOCAL_HOSTS.includes(url.hostname) && url.protocol === 'http:' ? { url: url.origin, token } : null;
  } catch {
    return null;
  }
}

/** "127.0.0.1:8787/" -> "http://127.0.0.1:8787". Throws on anything that is not an http(s) address. */
export function cleanBoxUrl(raw) {
  const text = (raw || '').trim();
  const parsed = new URL(/^https?:\/\//i.test(text) ? text : `http://${text}`);
  if (parsed.protocol !== 'http:' && parsed.protocol !== 'https:') throw new Error('Not a web address');
  return parsed.origin;
}

const toBoxId = (id) => (id === ROOT_ID ? '' : id);

export class HttpAdapter extends StorageAdapter {
  isBox = true;

  /** @param {{ url: string, token: string }} box */
  constructor(box) {
    super();
    this.url = box.url;
    this.token = box.token;
    this.thumbs = new Map(); // `${id}:${modified}` -> object URL
    this.texts = new Map(); // `${id}:${modified}` -> preview text
    this.art = new Map(); // `${system}:${game}:${kind}` -> object URL of arcade art
  }

  /** One request to the box. Throws an Error whose message is fit to show in a toast. */
  async call(path, init = {}) {
    let res;
    try {
      res = await fetch(this.url + API_BASE + path, {
        ...init,
        headers: { ...init.headers, Authorization: `Bearer ${this.token}` },
      });
    } catch {
      throw new Error(UNREACHABLE);
    }
    if (res.ok) return res;
    let message = `Box error ${res.status}`;
    try {
      message = (await res.json()).error || message;
    } catch {
      /* no JSON body: keep the status message */
    }
    throw new Error(message);
  }

  post(path, body) {
    return this.call(path, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(body),
    });
  }

  /** Box node -> UI node: the top folder is 'root' here, and previews are attached. */
  toNode(raw) {
    const node = { ...raw, parentId: raw.parentId === '' ? ROOT_ID : raw.parentId };
    const key = `${node.id}:${node.modified}`;
    if (this.thumbs.has(key)) node.thumb = this.thumbs.get(key);
    if (this.texts.has(key)) node.text = this.texts.get(key);
    return node;
  }

  /** Fetches thumbnails and text previews for small files, so the grid and preview work. */
  async attachPreviews(nodes) {
    await Promise.all(
      nodes.map(async (node) => {
        if (node.kind !== 'file' || node.trashed) return;
        const key = `${node.id}:${node.modified}`;
        const mime = node.mime || '';
        try {
          if (mime.startsWith('image/') && node.size <= THUMB_MAX_BYTES && !this.thumbs.has(key)) {
            const blob = await (await this.call(`/file?path=${encodeURIComponent(node.id)}`)).blob();
            this.thumbs.set(key, URL.createObjectURL(blob));
          }
          const isText = mime.startsWith('text/') || TEXT_MIMES.includes(mime);
          if (isText && node.size <= TEXT_MAX_BYTES && !this.texts.has(key)) {
            this.texts.set(key, await (await this.call(`/file?path=${encodeURIComponent(node.id)}`)).text());
          }
        } catch {
          return; // a missing preview is not worth failing the whole listing
        }
        if (this.thumbs.has(key)) node.thumb = this.thumbs.get(key);
        if (this.texts.has(key)) node.text = this.texts.get(key);
      })
    );
    return nodes;
  }

  async listChildren(parentId) {
    const res = await this.call(`/list?path=${encodeURIComponent(toBoxId(parentId))}`);
    return this.attachPreviews((await res.json()).map((n) => this.toNode(n)));
  }

  async all() {
    return (await (await this.call('/tree')).json()).map((n) => this.toNode(n));
  }

  async get(id) {
    try {
      return this.toNode(await (await this.call(`/stat?path=${encodeURIComponent(id)}`)).json());
    } catch {
      return null;
    }
  }

  async createFolder(name, parentId) {
    return this.toNode(await (await this.post('/mkdir', { parent: toBoxId(parentId), name })).json());
  }

  async rename(id, name) {
    return this.toNode(await (await this.post('/rename', { id, name })).json());
  }

  async toggleStar(id) {
    return this.toNode(await (await this.post('/star', { id })).json());
  }

  async trash(id) {
    await this.post('/trash', { id });
  }

  async restore(id) {
    await this.post('/restore', { id });
  }

  async deleteForever(id) {
    await this.post('/delete', { id });
  }

  async move(id, targetFolderId) {
    return this.toNode(await (await this.post('/move', { id, target: toBoxId(targetFolderId) })).json());
  }

  /** Sends each file straight from disk to the box; the box never overwrites, it renames. */
  async upload(files, parentId) {
    const made = [];
    for (const file of files) {
      const query = `parent=${encodeURIComponent(toBoxId(parentId))}&name=${encodeURIComponent(file.name || 'untitled')}`;
      const res = await this.call(`/upload?${query}`, { method: 'PUT', body: file });
      made.push(this.toNode(await res.json()));
    }
    return made;
  }

  objectUrl(node) {
    return this.thumbs.get(`${node.id}:${node.modified}`) || null;
  }

  /**
   * Saves a file from the box to this device. The whole file passes through memory first,
   * so very large downloads need a streaming path before this is the final version.
   */
  download(node) {
    this.call(`/file?path=${encodeURIComponent(node.id)}`)
      .then((res) => res.blob())
      .then((blob) => {
        const href = URL.createObjectURL(blob);
        const a = document.createElement('a');
        a.href = href;
        a.download = node.name;
        a.click();
        setTimeout(() => URL.revokeObjectURL(href), 60000);
      })
      .catch(() => {
        /* the store already said "Downloading"; a failed fetch leaves nothing saved */
      });
    return true;
  }

  async search(q) {
    const s = q.trim().toLowerCase();
    if (!s) return [];
    return (await this.all()).filter((n) => !n.trashed && n.name.toLowerCase().includes(s));
  }

  async recent(limit = 15) {
    return (await this.all())
      .filter((n) => !n.trashed && n.kind === 'file')
      .sort((a, b) => b.modified - a.modified)
      .slice(0, limit);
  }

  async stats() {
    return (await this.call('/stats')).json();
  }

  // ---- Clones: automations in Clonewars VMs (box started with --clonewars) ----

  async clones() {
    return (await this.call('/clones')).json();
  }

  /** Turns Clones on in a running box. The box finds its own clonewars folder. */
  async clonesEnable() {
    return (await this.call('/clones/enable', { method: 'POST' })).json();
  }

  /** Starts one bot of an automation. An empty bot means its first one. */
  async cloneStart(id, bot = '') {
    return (await this.post('/clones/start', { id, bot })).json();
  }

  async cloneStop(id, bot = '') {
    return (await this.post('/clones/stop', { id, bot })).json();
  }

  /** Creates an automation (no id) or changes one. Returns it as the box now has it. */
  async cloneAutomationSave(def) {
    return (await this.post('/clones/automation/save', def)).json();
  }

  /** @returns the automations that are left */
  async cloneAutomationDelete(id) {
    return (await this.post('/clones/automation/delete', { id })).json();
  }

  /** Adds a bot (empty bot id) or saves one's name and sign-in values. */
  async cloneBotSave(id, bot, name, values) {
    return (await this.post('/clones/bot/save', { id, bot, name, values })).json();
  }

  async cloneBotDelete(id, bot) {
    return (await this.post('/clones/bot/delete', { id, bot })).json();
  }

  /** The operating systems the box can boot, the machines it has, and recent launch times. */
  async cloneMachines() {
    return (await this.call('/clones/machines')).json();
  }

  /** body: { target, mode: existing|new|temporary, machine, name, memoryGb, cores, fresh, apt, pacman, rootCommands, userCommands, paths } */
  async cloneMachineStart(body) {
    return (await this.post('/clones/machine/start', body)).json();
  }

  async cloneMachineStop(id) {
    return (await this.post('/clones/machine/stop', { id })).json();
  }

  async cloneMachineDelete(id) {
    return (await this.post('/clones/machine/delete', { id })).json();
  }

  /** @returns {Promise<string>} an object URL of the machine's screen right now; revoke it when replaced */
  async cloneScreen(id) {
    const res = await this.call(`/clones/screen?path=${encodeURIComponent(id)}`);
    return URL.createObjectURL(await res.blob());
  }

  // ---- Network: the box's own connection ----

  async network() {
    return (await this.call('/network')).json();
  }

  /** Takes 10 to 30 seconds: the box downloads and uploads test data. */
  async networkSpeedTest() {
    return (await this.call('/network/speedtest', { method: 'POST' })).json();
  }

  async networkProvider() {
    return (await this.call('/network/provider')).json();
  }

  // ---- Settings kept on the box, and facts about the box ----

  async settings() {
    return (await this.call('/settings')).json();
  }

  async saveSettings(changes) {
    const res = await this.call('/settings', { method: 'PUT', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify(changes) });
    return res.json();
  }

  async boxInfo() {
    return (await this.call('/box')).json();
  }

  /** Processor, memory, disk and uptime of the machine the box runs on. */
  async system() {
    return (await this.call('/system')).json();
  }

  // ---- Security: checks, the activity record, the token, audits ----

  async security() {
    return (await this.call('/security')).json();
  }

  async securityLog(limit) {
    return (await this.call(`/security/log?limit=${limit}`)).json();
  }

  async securityVerify() {
    return (await this.call('/security/verify', { method: 'POST' })).json();
  }

  /** Replaces the box token and keeps this device connected with the new one. */
  async securityRotate() {
    const { token } = await (await this.call('/security/token/rotate', { method: 'POST' })).json();
    this.token = token;
    writeBox({ url: this.url, token });
  }

  async securityAudits() {
    return (await this.call('/security/audits')).json();
  }

  async securityAudit(id) {
    return (await this.post('/security/audits/run', { id })).json();
  }

  // ---- Arcade: the games folder on the box (box started with --arcade) ----

  async arcadeSystems() {
    return (await this.call('/arcade')).json();
  }

  // ---- Arcade save data: every save file, its game, and every version recorded ----

  async arcadeSaves() {
    return (await this.call('/arcade/saves')).json();
  }

  /** Records every save that changed since it was last recorded. Reads the drive only. */
  async arcadeSavesRecord() {
    return (await this.call('/arcade/saves/record', { method: 'POST' })).json();
  }

  async arcadeSaveHistory(id) {
    return (await this.call(`/arcade/saves/history?path=${encodeURIComponent(id)}`)).json();
  }

  /** Puts a recorded version back in place of the save file on the games drive. */
  async arcadeSaveRestore(id, hash) {
    return (await this.post('/arcade/saves/restore', { id, hash })).json();
  }

  /** Turns Arcade on in a running box. The box looks for a Games folder on its drives. */
  async arcadeEnable() {
    return (await this.call('/arcade/enable', { method: 'POST' })).json();
  }

  async arcadeGames(system) {
    return (await this.call(`/arcade/games?system=${encodeURIComponent(system)}`)).json();
  }

  /** @returns {Promise<string | null>} an object URL for the game's image, or null if it has none */
  async arcadeArt(system, game, kind = 'boxart') {
    const key = `${system}:${game}:${kind}`;
    if (this.art.has(key)) return this.art.get(key);
    try {
      const query = `system=${encodeURIComponent(system)}&game=${game}&kind=${kind}`;
      const url = URL.createObjectURL(await (await this.call(`/arcade/art?${query}`)).blob());
      this.art.set(key, url);
      return url;
    } catch {
      return null;
    }
  }

  async arcadePlay(system, game) {
    return (await this.post('/arcade/play', { system, game })).json();
  }

  /** Frees every preview held in memory; call when leaving this box. */
  dispose() {
    for (const url of this.thumbs.values()) URL.revokeObjectURL(url);
    for (const url of this.art.values()) URL.revokeObjectURL(url);
    this.thumbs.clear();
    this.texts.clear();
    this.art.clear();
  }
}

/**
 * Checks an address and token against a live box, and remembers them if they work.
 * @returns {Promise<HttpAdapter>}
 */
export async function connectBox(rawUrl, rawToken) {
  const box = { url: cleanBoxUrl(rawUrl), token: (rawToken || '').trim() };
  if (!box.token) throw new Error('The box token is required');
  const adapter = new HttpAdapter(box);
  await adapter.stats(); // proves both the address and the token
  writeBox(box);
  return adapter;
}

/** True inside the Tauri desktop or mobile shell, false in a browser. */
export function isDesktopApp() {
  return typeof window !== 'undefined' && !!window.__TAURI_INTERNALS__;
}

/**
 * Desktop app only: asks the shell for the box it runs on this computer, starting it if needed.
 * @returns {Promise<{ url: string, token: string }>}
 */
export async function localBox() {
  const { invoke } = await import('@tauri-apps/api/core');
  return invoke('local_box');
}

/** The fixed sample drive behind /demo: same files every visit, nothing saved. */
export function fixedDemoAdapter(person = null) {
  const adapter = new LocalStorageAdapter({ nodes: demoNodes(person) });
  adapter.isDemo = true;
  return adapter;
}

/** The backend to start with: the /demo drive, the remembered box, or the local drive. */
export function startingAdapter() {
  if (isDemoRoute()) return fixedDemoAdapter();
  const box = readBox();
  if (box) return new HttpAdapter(box);
  const adapter = new LocalStorageAdapter();
  adapter.isNative = isDesktopApp();
  return adapter;
}

/** The drive kept in this browser's storage, used when no box is connected. */
export function localAdapter() {
  return new LocalStorageAdapter();
}
