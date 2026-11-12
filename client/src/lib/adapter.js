// Storage adapter interface + localStorage demo backend.
// The Tauri shell swaps in a native adapter later; the UI only talks to this contract.

import { uid } from './format.js';
import { isHostId } from './computer.js';

const HOST_TEST_MS = 1600; // how long the demo's drive speed test pretends to take

/**
 * Node: { id, name, kind: 'folder'|'file', parentId, mime, size, modified,
 *          starred, trashed, shared, thumb?, text? }
 */
export class StorageAdapter {
  async listChildren(_parentId) { throw new Error('not implemented'); }
  async get(_id) { throw new Error('not implemented'); }
  async createFolder(_name, _parentId) { throw new Error('not implemented'); }
  async rename(_id, _name) { throw new Error('not implemented'); }
  async toggleStar(_id) { throw new Error('not implemented'); }
  async trash(_id) { throw new Error('not implemented'); }
  async restore(_id) { throw new Error('not implemented'); }
  async deleteForever(_id) { throw new Error('not implemented'); }
  async move(_id, _targetFolderId) { throw new Error('not implemented'); }
  async upload(_files, _parentId) { throw new Error('not implemented'); }
  async search(_q) { throw new Error('not implemented'); }
  async recent(_limit) { throw new Error('not implemented'); }
  async all() { throw new Error('not implemented'); }
  async stats() { throw new Error('not implemented'); }
  /** Object URL for preview/download when the backend has real bytes. */
  objectUrl(_node) { return _node.thumb || null; }
}

const STORE_KEY = 'castle-drive-v1';
const QUOTA = 15 * 1024 ** 3; // 15 GB demo quota
const FIXED_QUOTA = 2 * 1024 ** 4; // 2 TB: the /demo drive holds sample files far bigger than 15 GB

/** A labelled gradient standing in for a photo, so sample files have thumbnails. */
export const svgThumb = (label, c1, c2) =>
  'data:image/svg+xml;utf8,' +
  encodeURIComponent(
    `<svg xmlns="http://www.w3.org/2000/svg" width="400" height="300">` +
      `<defs><linearGradient id="g" x1="0" y1="0" x2="1" y2="1">` +
      `<stop offset="0" stop-color="${c1}"/><stop offset="1" stop-color="${c2}"/>` +
      `</linearGradient></defs>` +
      `<rect width="400" height="300" fill="url(#g)"/>` +
      `<text x="18" y="282" font-family="monospace" font-size="15" fill="rgba(255,255,255,.85)">${label}</text></svg>`
  );

function seed() {
  const now = Date.now();
  const day = 864e5;
  const H = 36e5;
  const n = (o) => ({ id: uid(), kind: 'file', parentId: 'root', mime: 'application/octet-stream', size: 0, modified: now - 3 * day, starred: false, trashed: false, shared: false, ...o });
  const f = (name, parentId, extra = {}) => n({ kind: 'folder', name, parentId, size: 0, mime: '', ...extra });

  const projects = f('Projects', 'root', { modified: now - 2 * day });
  const photos = f('Photos', 'root', { modified: now - 5 * day });
  const music = f('Music', 'root', { modified: now - 9 * day });
  const backups = f('Backups', 'root', { modified: now - 14 * day });
  const bridge = f('Bridge Files', 'root', { modified: now - 1 * day, shared: true });

  return [
    projects, photos, music, backups, bridge,
    n({ name: 'castle-schematic.svg', parentId: projects.id, mime: 'image/svg+xml', size: 184320, modified: now - 2 * H, thumb: svgThumb('CASTLE // SCHEMATIC v3', '#1a2b4a', '#050508') }),
    n({ name: 'field-notes.txt', parentId: projects.id, mime: 'text/plain', size: 4218, modified: now - 5 * H, starred: true, text: 'FIELD NOTES — 09/10\n\n- Chiral readings nominal.\n- Uplink stable at 940/880.\n- The shelf goes crazy.\n- Pack light. Move fast.\n' }),
    n({ name: 'pitch-v3.pdf', parentId: projects.id, mime: 'application/pdf', size: 2_410_112, modified: now - 26 * H }),
    n({ name: 'dune-trip.jpg', parentId: photos.id, mime: 'image/jpeg', size: 3_881_920, modified: now - 5 * day, thumb: svgThumb('DUNE TRIP', '#8a5a2b', '#2b1a0e') }),
    n({ name: 'neon-city.jpg', parentId: photos.id, mime: 'image/jpeg', size: 4_202_496, modified: now - 6 * day, thumb: svgThumb('NEON CITY', '#123a4a', '#0e0b1e') }),
    n({ name: 'loop-04.wav', parentId: music.id, mime: 'audio/wav', size: 44_120_064, modified: now - 9 * day }),
    n({ name: 'demo-take-2.mp3', parentId: music.id, mime: 'audio/mpeg', size: 9_830_400, modified: now - 10 * day, starred: true }),
    n({ name: 'phone-2026-08.zip', parentId: backups.id, mime: 'application/zip', size: 8_120_422_400, modified: now - 14 * day }),
    n({ name: 'uplink-manifest.md', parentId: bridge.id, mime: 'text/markdown', size: 1892, modified: now - 20 * H, shared: true, text: '# UPLINK MANIFEST\n\n- [x] Bridge handshake\n- [x] Cargo verified\n- [ ] Return trip\n' }),
    n({ name: 'drop-07.bin', parentId: bridge.id, mime: 'application/octet-stream', size: 671_088_640, modified: now - 2 * day, shared: true }),
    n({ name: 'old-resume.pdf', parentId: 'root', mime: 'application/pdf', size: 98_304, modified: now - 60 * day, trashed: true }),
  ];
}

export class LocalStorageAdapter extends StorageAdapter {
  /**
   * @param {{ nodes?: object[], computer?: object }} [options] pass `nodes` for a fixed, memory-only drive
   *   (the /demo route): it starts from exactly those nodes and saves nothing. `computer`
   *   adds the demo's pretend computer (see demoComputer in demo.js).
   */
  constructor(options = {}) {
    super();
    this.blobs = new Map(); // id -> Blob (memory only; real bytes live here until native fs lands)
    this.objectUrls = new Map();
    this.nodes = new Map();
    this.fixed = !!options.nodes;
    // The demo's pretend computer: drives, and folders on them as nodes with "host:" ids.
    if (options.computer) {
      this.computer = options.computer;
      this.hostDrives = async () => structuredClone({ ...this.computer, nodes: undefined, at: Date.now() });
      this.hostSpeedTest = (path, write = true) => this.#fakeSpeedTest(path, write);
    }
    if (this.fixed) {
      for (const node of options.nodes) this.nodes.set(node.id, node);
      return;
    }
    this.#load();
  }

  #load() {
    try {
      const raw = localStorage.getItem(STORE_KEY);
      if (raw) {
        for (const node of JSON.parse(raw)) this.nodes.set(node.id, node);
        return;
      }
    } catch { /* corrupted demo db → reseed */ }
    for (const node of seed()) this.nodes.set(node.id, node);
    this.#save();
  }

  #save() {
    if (this.fixed) return;
    try {
      localStorage.setItem(STORE_KEY, JSON.stringify([...this.nodes.values()]));
    } catch { /* quota — demo keeps running in memory */ }
  }

  #kids(parentId) {
    return [...this.nodes.values()].filter((n) => n.parentId === parentId && !n.trashed);
  }

  /** Vault nodes only: the computer's files are not in Recent, Starred, search or the meter. */
  #vault() {
    return [...this.nodes.values()].filter((n) => !isHostId(n.id));
  }

  async #fakeSpeedTest(path, write) {
    const drive = this.computer.drives.find((d) => d.path === path);
    if (!drive) throw new Error('That drive is not plugged in.');
    if (write && drive.readOnly) throw new Error('This drive cannot be written to. Run the read-only test instead.');
    await new Promise((resolve) => setTimeout(resolve, HOST_TEST_MS));
    const base = { fixed: 2400, external: 410, removable: 92, optical: 21 }[drive.kind] || 100;
    const wobble = () => 0.9 + Math.random() * 0.2;
    const result = {
      at: Date.now(),
      mode: write ? 'write' : 'read',
      writeMbps: write ? Math.round(base * 0.55 * wobble() * 10) / 10 : null,
      readMbps: Math.round(base * wobble() * 10) / 10,
      bytes: 256 * 1024 * 1024,
    };
    drive.lastTest = result;
    return result;
  }

  async listChildren(parentId) { return this.#kids(parentId); }
  async get(id) { return this.nodes.get(id) || null; }
  async all() { return this.#vault(); }

  async createFolder(name, parentId) {
    const node = { id: this.#newId(parentId, name), name, kind: 'folder', parentId, mime: '', size: 0, modified: Date.now(), starred: false, trashed: false, shared: false };
    this.nodes.set(node.id, node);
    this.#save();
    return node;
  }

  async rename(id, name) {
    const node = this.nodes.get(id);
    if (!node) return null;
    node.name = name;
    node.modified = Date.now();
    this.#save();
    return node;
  }

  async toggleStar(id) {
    if (isHostId(id)) throw new Error('Stars are for files in My Drive');
    const node = this.nodes.get(id);
    if (!node) return null;
    node.starred = !node.starred;
    this.#save();
    return node;
  }

  async trash(id) {
    // The demo computer has no Recycle Bin to look in: a file sent there is just gone.
    if (isHostId(id)) return this.deleteForever(id);
    const node = this.nodes.get(id);
    if (!node) return;
    // trashing a folder trashes its subtree
    const sweep = (nid) => {
      const n = this.nodes.get(nid);
      if (!n) return;
      n.trashed = true;
      for (const k of this.#kids(nid)) sweep(k.id);
      for (const k of [...this.nodes.values()].filter((x) => x.parentId === nid && x.trashed)) sweep(k.id);
    };
    sweep(id);
    this.#save();
  }

  async restore(id) {
    const node = this.nodes.get(id);
    if (!node) return;
    const sweep = (nid) => {
      const n = this.nodes.get(nid);
      if (!n) return;
      n.trashed = false;
      for (const k of [...this.nodes.values()].filter((x) => x.parentId === nid)) sweep(k.id);
    };
    sweep(id);
    this.#save();
  }

  async deleteForever(id) {
    const gone = (nid) => {
      for (const k of [...this.nodes.values()].filter((x) => x.parentId === nid)) gone(k.id);
      this.nodes.delete(nid);
      this.blobs.delete(nid);
      if (this.objectUrls.get(nid)) URL.revokeObjectURL(this.objectUrls.get(nid));
      this.objectUrls.delete(nid);
    };
    gone(id);
    this.#save();
  }

  async move(id, targetFolderId) {
    const node = this.nodes.get(id);
    if (!node || id === targetFolderId) return null;
    node.parentId = targetFolderId;
    node.modified = Date.now();
    this.#save();
    return node;
  }

  async upload(files, parentId) {
    const made = [];
    for (const file of files) {
      const node = {
        id: this.#newId(parentId, file.name || 'untitled'), name: file.name || 'untitled', kind: 'file', parentId,
        mime: file.type || 'application/octet-stream', size: file.size || 0,
        modified: Date.now(), starred: false, trashed: false, shared: false,
      };
      this.blobs.set(node.id, file);
      if (node.mime.startsWith('image/')) {
        const url = URL.createObjectURL(file);
        this.objectUrls.set(node.id, url);
        node.thumb = url;
      } else if (node.mime.startsWith('text/') || node.mime === 'application/json') {
        try { node.text = await file.text().then((t) => t.slice(0, 20000)); } catch { /* binary-ish */ }
      }
      this.nodes.set(node.id, node);
      made.push(node);
    }
    this.#save();
    return made;
  }

  objectUrl(node) {
    return this.objectUrls.get(node.id) || node.thumb || null;
  }

  download(node) {
    const blob = this.blobs.get(node.id);
    if (!blob) return false;
    const a = document.createElement('a');
    a.href = this.objectUrls.get(node.id) || URL.createObjectURL(blob);
    a.download = node.name;
    a.click();
    return true;
  }

  /** A computer item's id is its path; a vault item's is random. */
  #newId(parentId, name) {
    if (!isHostId(parentId)) return uid();
    const sep = parentId.includes('/') ? '/' : '\\';
    const base = parentId.endsWith(sep) ? parentId : parentId + sep;
    let id = base + name;
    for (let n = 2; this.nodes.has(id); n++) id = `${base}${name} (${n})`;
    return id;
  }

  async search(q) {
    const s = q.trim().toLowerCase();
    if (!s) return [];
    return this.#vault().filter((n) => !n.trashed && n.name.toLowerCase().includes(s));
  }

  async recent(limit = 15) {
    return this.#vault()
      .filter((n) => !n.trashed && n.kind === 'file')
      .sort((a, b) => b.modified - a.modified)
      .slice(0, limit);
  }

  async stats() {
    let used = 0;
    for (const n of this.#vault()) if (!n.trashed && n.kind === 'file') used += n.size || 0;
    return { used, quota: this.fixed ? FIXED_QUOTA : QUOTA };
  }
}

// Which backend runs is decided in box.js (startingAdapter): a connected box, or this demo.
