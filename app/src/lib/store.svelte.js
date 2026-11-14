// DriveStore — all drive state + actions in one reactive class.
// The UI is a thin skin over this. Keyboard shortcuts call these directly,
// so everything here must feel instant: no awaits that block the first paint.

import { startingAdapter, localAdapter, fixedDemoAdapter, connectBox, forgetBox, takeConnectLink, isDesktopApp, localBox } from './box.js';
import { SvelteSet } from 'svelte/reactivity';
import { uid } from './format.js';
import { ask } from './ask.svelte.js';
import { prefs as devicePrefs } from './prefs.svelte.js';
import { HOST_ROOT, COMPUTER_NAME, isHostId, hostTrail, hostParent } from './computer.js';
import { SHOWCASE, REFUSED, onRefused, refused } from './showcase.js';

export const VIEWS = ['drive', 'computer', 'shared', 'recent', 'starred', 'trash'];
export const SORT_KEYS = ['name', 'size', 'modified'];
export const SORT_LABELS = { name: 'NAME', size: 'SIZE', modified: 'MODIFIED' };

// ---- per-session drive prefs (sort + density). sessionStorage only, no PII. ----
const PREFS_KEY = 'castle-drive-prefs';
const GENERIC_ERROR = 'Something went wrong';
// Actions that change the drive. Each is wrapped below so a failure (box unreachable, name
// taken, token rejected) becomes a toast and a fresh listing instead of a silent dead end.
const GUARDED_ACTIONS = [
  'newFolder', 'commitRename', 'trashSelection', 'restoreSelection', 'deleteForeverSelection',
  'toggleStar', 'moveSelection', 'uploadFiles', 'setView', 'openNode', 'goUp', 'goToCrumb', 'setQuery',
  'climb', 'openHost', 'speedTest',
];
// The showcase (see showcase.js) refuses these before they start: no rename box opens, no
// file picker appears, nothing is half done. The adapter refuses the same calls as well.
const SHOWCASE_REFUSED = [
  'newFolder', 'startRename', 'trashSelection', 'restoreSelection', 'deleteForeverSelection',
  'toggleStar', 'moveSelection', 'uploadFiles', 'speedTest', 'connect', 'disconnect',
];
function readPrefs() {
  try {
    return JSON.parse(sessionStorage.getItem(PREFS_KEY)) || {};
  } catch {
    return {};
  }
}
function writePrefs(prefs) {
  try {
    sessionStorage.setItem(PREFS_KEY, JSON.stringify(prefs));
  } catch {
    /* storage full/blocked — prefs just won't persist */
  }
}

class DriveStore {
  adapter = startingAdapter();
  boxUrl = $state(this.adapter.isBox ? this.adapter.url : null); // null = not on a box
  isDemo = $state(!!this.adapter.isDemo); // true on /demo: fixed sample files, nothing saved
  connectOpen = $state(false);

  view = $state('drive');
  path = $state([{ id: 'root', name: 'My Drive' }]);
  items = $state([]);
  selected = new SvelteSet(); // reactive: add/delete/clear update the UI
  focusId = $state(null);
  selAnchor = null;
  viewMode = $state('grid'); // 'grid' | 'list' — density toggle (compact grid / comfortable list)
  sortKey = $state('name'); // 'name' | 'size' | 'modified'
  sortDir = $state(1); // 1 = ascending, -1 = descending
  query = $state('');
  renamingId = $state(null);
  previewId = $state(null);
  menu = $state(null); // { x, y, id } — context menu
  newMenuOpen = $state(false);
  toasts = $state([]);
  dragDepth = $state(0);
  slideDir = $state(1); // folder drill direction for transitions
  columnsProvider = null; // set by FileBrowser: () => visible columns
  // This computer: { vault, separator, drives, gone, at } from the box, or null when this
  // backend has no computer to show (a browser-only drive) or the box does not allow it.
  computer = $state(null);
  computerOff = $state(''); // why not, in words, when the box said no
  testing = $state(null); // "<drive path>|write" or "|read" while a speed test runs

  get cwd() {
    return this.path[this.path.length - 1];
  }

  get isTrash() {
    return this.view === 'trash';
  }

  /** A view with folders to walk through: My Drive, or This computer. */
  get browsing() {
    return this.view === 'drive' || this.view === 'computer';
  }

  /** On the drive list at the top of This computer. */
  get atComputerTop() {
    return this.view === 'computer' && this.cwd.id === HOST_ROOT;
  }

  /** The "../" tile: at the top of My Drive (up into the computer) and in any computer folder. */
  get canClimb() {
    if (this.query) return false;
    if (this.view === 'drive') return this.path.length === 1 && !!this.computer;
    return this.view === 'computer' && this.cwd.id !== HOST_ROOT;
  }

  /** Where "../" leads, by name. */
  get climbName() {
    if (this.view === 'drive') {
      const up = this.computer ? hostTrail(hostParent(this.computer.vault)) : [];
      return up.length ? up[up.length - 1].name : COMPUTER_NAME;
    }
    return this.path.length > 1 ? this.path[this.path.length - 2].name : '';
  }

  /** "Recycle Bin" on Windows, "bin" elsewhere. */
  get binName() {
    return this.computer?.separator === '/' ? 'the bin' : 'the Recycle Bin';
  }

  /** Boot: restore session prefs, then load the current view. */
  async init() {
    const prefs = readPrefs();
    if (SORT_KEYS.includes(prefs.sortKey)) this.sortKey = prefs.sortKey;
    if (prefs.sortDir === 1 || prefs.sortDir === -1) this.sortDir = prefs.sortDir;
    // This visit's choice wins; otherwise Settings > Storage > Show files as.
    this.viewMode = prefs.viewMode === 'grid' || prefs.viewMode === 'list' ? prefs.viewMode : devicePrefs.get('storage.fileView');
    const link = takeConnectLink();
    if (link) {
      try {
        await this.connect(link.url, link.token);
        return;
      } catch (err) {
        this.toast(err?.message || GENERIC_ERROR);
      }
    }
    if (await this.connectLocalBox()) return;
    this.loadComputer();
    try {
      await this.refresh();
    } catch (err) {
      this.toast(err?.message || GENERIC_ERROR);
    }
  }

  /**
   * In the desktop app: connect to the box the app runs on this computer. Skipped on
   * /demo, in a browser, and when this device already remembers a different box.
   * @returns {Promise<boolean>} true when it connected
   */
  async connectLocalBox() {
    if (this.isDemo || !isDesktopApp()) return false;
    try {
      const box = await localBox();
      if (this.boxUrl && this.boxUrl !== box.url) return false;
      await this.connect(box.url, box.token, { quiet: true });
      return true;
    } catch (err) {
      this.toast(typeof err === 'string' ? err : err?.message || GENERIC_ERROR);
      return false;
    }
  }

  /** Switch to a real box. Throws (with a message fit to show) if the address or token is wrong. */
  async connect(url, token, { quiet = false } = {}) {
    const adapter = await connectBox(url, token);
    // Connecting from /demo leaves the demo: a reload must not land back on sample files.
    if (this.isDemo) history.replaceState(null, '', '/');
    this.useAdapter(adapter);
    this.boxUrl = adapter.url;
    this.connectOpen = false;
    await this.refresh();
    if (!quiet) this.toast('Connected');
  }

  /**
   * Switches between your own data and the sample data without reloading. Going live
   * returns to the box this device remembers, or in the desktop app to the box it runs.
   * In a browser the address follows (/demo), so a reload stays in the same mode.
   */
  async setDemo(on) {
    if (on === this.isDemo || SHOWCASE) return; // the showcase is only ever the demo
    if (!isDesktopApp()) history.replaceState(null, '', (on ? '/demo' : '/') + location.search);
    this.connectOpen = false;
    if (on) {
      this.useAdapter(fixedDemoAdapter());
      this.boxUrl = null;
    } else {
      const adapter = startingAdapter();
      this.useAdapter(adapter);
      this.boxUrl = adapter.isBox ? adapter.url : null;
      if (await this.connectLocalBox()) return;
    }
    try {
      await this.refresh();
    } catch (err) {
      this.toast(err?.message || GENERIC_ERROR);
    }
  }

  /** Back to the local drive; the token is forgotten on this device. */
  async disconnect() {
    forgetBox();
    this.useAdapter(localAdapter());
    this.boxUrl = null;
    this.connectOpen = false;
    await this.refresh();
    this.toast('Disconnected');
  }

  /** Shows one person's drive. Only the demo has more than one person so far. */
  async showDriveOf(person) {
    if (!this.isDemo) return;
    this.useAdapter(fixedDemoAdapter(person));
    await this.refresh();
  }

  useAdapter(adapter) {
    this.adapter.dispose?.();
    this.adapter = adapter;
    this.isDemo = !!adapter.isDemo;
    this.view = 'drive';
    this.path = [{ id: 'root', name: 'My Drive' }];
    this.query = '';
    this.previewId = null;
    this.computer = null;
    this.computerOff = '';
    this.clearSelection();
    this.loadComputer();
  }

  savePrefs() {
    writePrefs({ sortKey: this.sortKey, sortDir: this.sortDir, viewMode: this.viewMode });
  }

  /** Sort control: name → size → modified, ascending + descending. */
  cycleSortKey() {
    this.sortKey = SORT_KEYS[(SORT_KEYS.indexOf(this.sortKey) + 1) % SORT_KEYS.length];
    this.savePrefs();
    this.resort();
  }

  toggleSortDir() {
    this.sortDir = -this.sortDir;
    this.savePrefs();
    this.resort();
  }

  /** Density toggle: compact grid ↔ comfortable list. */
  setViewMode(mode) {
    if (mode !== 'grid' && mode !== 'list') return;
    this.viewMode = mode;
    this.savePrefs();
  }

  /** Re-run the current item order after a sort/direction change (no fetch). */
  resort() {
    this.items = [...this.items].sort(this.compareNodes);
    const ids = new Set(this.items.map((i) => i.id));
    for (const id of [...this.selected]) if (!ids.has(id)) this.selected.delete(id);
    if (this.focusId && !ids.has(this.focusId)) this.focusId = null;
    if (this.renamingId && !ids.has(this.renamingId)) this.renamingId = null;
  }

  compareNodes(a, b) {
    if (this.view === 'recent') return b.modified - a.modified;
    const dir = this.sortDir;
    if (this.sortKey === 'name') {
      if (a.kind !== b.kind) return a.kind === 'folder' ? -1 : 1;
      return dir * a.name.localeCompare(b.name, undefined, { sensitivity: 'base' });
    }
    if (this.sortKey === 'size') {
      const sa = a.kind === 'folder' ? -1 : a.size || 0;
      const sb = b.kind === 'folder' ? -1 : b.size || 0;
      if (sa !== sb) return dir * (sa - sb);
      return a.name.localeCompare(b.name, undefined, { sensitivity: 'base' });
    }
    // modified
    if (a.modified !== b.modified) return dir * (a.modified - b.modified);
    if (a.kind !== b.kind) return a.kind === 'folder' ? -1 : 1;
    return 0;
  }

  async refresh() {
    let items;
    if (this.view === 'computer') {
      items = await this.listComputer();
    } else if (this.query.trim()) {
      items = await this.adapter.search(this.query);
    } else if (this.view === 'drive') {
      items = await this.adapter.listChildren(this.cwd.id);
    } else if (this.view === 'shared') {
      items = (await this.adapter.all()).filter((n) => n.shared && !n.trashed);
    } else if (this.view === 'recent') {
      items = await this.adapter.recent(20);
    } else if (this.view === 'starred') {
      items = (await this.adapter.all()).filter((n) => n.starred && !n.trashed);
    } else if (this.view === 'trash') {
      items = (await this.adapter.all()).filter((n) => n.trashed);
    }
    items.sort(this.compareNodes.bind(this));
    this.items = items;
    // drop selection/focus for nodes that vanished
    const ids = new Set(items.map((i) => i.id));
    for (const id of [...this.selected]) if (!ids.has(id)) this.selected.delete(id);
    if (this.focusId && !ids.has(this.focusId)) this.focusId = null;
    if (this.renamingId && !ids.has(this.renamingId)) this.renamingId = null;
  }

  /** The open computer folder, filtered by the search box and the hidden-files setting. */
  async listComputer() {
    if (this.cwd.id === HOST_ROOT) {
      await this.loadComputer();
      return [];
    }
    let items = await this.adapter.listChildren(this.cwd.id);
    if (!devicePrefs.get('storage.showHidden')) items = items.filter((n) => !n.hidden);
    const q = this.query.trim().toLowerCase();
    return q ? items.filter((n) => n.name.toLowerCase().includes(q)) : items;
  }

  /**
   * Asks the box about the computer's drives. Says so when a drive has just been plugged in.
   * Leaves `computer` null when this backend has none or the box does not allow it.
   */
  async loadComputer() {
    if (!this.adapter.hostDrives) {
      this.computer = null;
      return;
    }
    const adapter = this.adapter;
    try {
      const next = await adapter.hostDrives();
      if (adapter !== this.adapter) return; // switched box while asking
      const before = this.computer ? new Set(this.computer.drives.map((d) => d.key)) : null;
      if (before) {
        for (const drive of next.drives) {
          if (drive.removable && !before.has(drive.key)) this.toast(`${drive.name} plugged in`);
        }
      }
      this.computer = next;
      this.computerOff = '';
    } catch (err) {
      if (adapter !== this.adapter) return;
      this.computer = null;
      this.computerOff = err?.message || GENERIC_ERROR;
    }
  }

  /** "../": from the top of My Drive into the folder the vault is in, then up and up. */
  async climb() {
    if (!this.canClimb) return;
    if (this.view === 'drive') {
      await this.openHost(hostParent(this.computer.vault), -1);
      return;
    }
    this.slideDir = -1;
    this.path = this.path.slice(0, -1);
    this.clearSelection();
    await this.refresh();
  }

  /** Opens a folder on the computer by its path; '' opens the drive list. */
  async openHost(path, dir = 1) {
    this.view = 'computer';
    this.query = '';
    this.slideDir = dir;
    this.path = hostTrail(path);
    this.clearSelection();
    await this.refresh();
  }

  /** Times a drive. `write` false reads only, for a drive that cannot be written. */
  async speedTest(drive, write = true) {
    if (this.testing) return;
    this.testing = `${drive.path}|${write ? 'write' : 'read'}`;
    try {
      const result = await this.adapter.hostSpeedTest(drive.path, write);
      const read = `read ${result.readMbps} MB/s`;
      this.toast(result.writeMbps != null ? `${drive.name}: write ${result.writeMbps} MB/s, ${read}` : `${drive.name}: ${read}`);
      await this.loadComputer();
    } finally {
      this.testing = null;
    }
  }

  // ---------- navigation ----------
  async setView(v) {
    if (this.view === v && !(v === 'computer' && !this.atComputerTop)) return;
    this.view = v;
    this.slideDir = 1;
    if (v === 'drive') this.path = [{ id: 'root', name: 'My Drive' }];
    if (v === 'computer') this.path = hostTrail('');
    // Recent, Starred and the rest add new files to My Drive, never to a computer folder.
    else if (isHostId(this.cwd.id)) this.path = [{ id: 'root', name: 'My Drive' }];
    this.query = '';
    this.clearSelection();
    await this.refresh();
  }

  async openNode(node) {
    if (!node) return;
    if (node.kind === 'folder') {
      this.view = isHostId(node.id) ? 'computer' : 'drive';
      this.query = '';
      this.slideDir = 1;
      this.path = [...this.path, { id: node.id, name: node.name }];
      this.clearSelection();
      await this.refresh();
    } else {
      this.previewId = node.id;
    }
  }

  async goUp() {
    if (!this.browsing || this.query) return;
    if (this.path.length <= 1) {
      if (this.canClimb) await this.climb(); // Backspace at the top of My Drive is "../" too
      return;
    }
    this.slideDir = -1;
    this.path = this.path.slice(0, -1);
    this.clearSelection();
    await this.refresh();
  }

  async goToCrumb(i) {
    if (!this.browsing) return;
    this.slideDir = -1;
    this.path = this.path.slice(0, i + 1);
    this.query = '';
    this.clearSelection();
    await this.refresh();
  }

  // ---------- selection ----------
  select(id, additive = false, range = false) {
    if (range && this.selAnchor) {
      const ids = this.items.map((i) => i.id);
      const a = ids.indexOf(this.selAnchor);
      const b = ids.indexOf(id);
      if (a !== -1 && b !== -1) {
        const [lo, hi] = [Math.min(a, b), Math.max(a, b)];
        for (let k = lo; k <= hi; k++) this.selected.add(ids[k]);
      }
    } else if (additive) {
      this.selected.has(id) ? this.selected.delete(id) : this.selected.add(id);
    } else {
      this.selected.clear();
      this.selected.add(id);
    }
    this.focusId = id;
    if (!range) this.selAnchor = id;
  }

  selectAll() {
    this.selected.clear();
    for (const item of this.items) this.selected.add(item.id);
    if (this.items.length) this.focusId = this.items[0].id;
  }

  clearSelection() {
    this.selected.clear();
    this.focusId = null;
    this.selAnchor = null;
  }

  /** Moves focus through the files. Returns false at the edge (nothing that way), so
      focus can leave the grid for the rail, the bar or the "../" tile. */
  moveFocus(key, extend = false) {
    if (!this.items.length) return false;
    const ids = this.items.map((i) => i.id);
    let idx = ids.indexOf(this.focusId);
    if (idx === -1) idx = 0;
    const cols = this.viewMode === 'grid' ? (this.columnsProvider?.() || 4) : 1;
    const row = Math.floor(idx / cols);
    const lastRow = Math.floor((ids.length - 1) / cols);
    if (key === 'ArrowRight') {
      if (cols === 1 || idx % cols === cols - 1 || idx === ids.length - 1) return false;
      idx += 1;
    } else if (key === 'ArrowLeft') {
      if (cols === 1 || idx % cols === 0) return false;
      idx -= 1;
    } else if (key === 'ArrowDown') {
      if (row === lastRow) return false;
      idx = Math.min(idx + cols, ids.length - 1);
    } else if (key === 'ArrowUp') {
      if (row === 0) return false;
      idx -= cols;
    } else return false;
    const id = ids[idx];
    if (extend) {
      if (!this.selAnchor) this.selAnchor = this.focusId || ids[0];
      this.select(id, false, true);
    } else {
      this.select(id);
    }
    this.focusId = id;
    this.ensureVisible(id);
    return true;
  }

  /** Auto-scroll the node into view and hand it DOM focus (roving tabindex). */
  ensureVisible(id) {
    const reveal = () => {
      const el = document.querySelector(`[data-node="${id}"]`);
      el?.scrollIntoView({ block: 'nearest' });
      if (document.activeElement?.closest?.('[data-kb]')) el?.focus({ preventScroll: true });
      return !!el;
    };
    // At once when the file is on screen, so a quick second key starts from it; a frame
    // later when it is still being drawn (just created, just uploaded).
    if (!reveal()) requestAnimationFrame(reveal);
  }

  /** Type-ahead: jump to the first node matching the typed prefix (Drive behavior).
      A fast repeated single char cycles through its matches. */
  jumpToPrefix(char) {
    if (!this.items.length || char.length !== 1) return;
    const now = Date.now();
    if (now - (this.typeAt || 0) > 900) this.typeBuffer = '';
    this.typeAt = now;
    const c = char.toLowerCase();
    this.typeBuffer = (this.typeBuffer || '') + c;
    if (this.typeBuffer.length > 24) this.typeBuffer = c;
    const buf = this.typeBuffer;

    let target = null;
    if (buf.length === 1) {
      // cycle through matches on repeat
      const matches = this.items.filter((n) => n.name.toLowerCase().startsWith(buf));
      if (matches.length) {
        const ids = matches.map((m) => m.id);
        const cur = ids.indexOf(this.focusId);
        target = matches[(cur + 1) % matches.length];
      }
    } else {
      target = this.items.find((n) => n.name.toLowerCase().startsWith(buf)) || null;
    }
    if (target) {
      this.select(target.id);
      this.ensureVisible(target.id);
    }
  }

  activateFocus() {
    const node = this.items.find((i) => i.id === this.focusId);
    if (node) this.openNode(node);
  }

  // ---------- mutations (all instant-feeling) ----------
  async newFolder() {
    if (this.atComputerTop) return this.toast('Open a drive first');
    const node = await this.adapter.createFolder('New folder', this.cwd.id);
    this.query = '';
    await this.refresh();
    // appear + editable in the same beat
    this.select(node.id);
    this.renamingId = node.id;
  }

  startRename(id = this.focusId) {
    if (!id) return;
    this.renamingId = id;
    this.menu = null;
  }

  async commitRename(id, name) {
    if (this.renamingId !== id) return; // Enter then blur both commit: only the first one counts
    const clean = (name || '').trim();
    this.renamingId = null;
    if (!clean) { await this.refresh(); return; } // Esc/empty → keep old name
    const node = await this.adapter.get(id);
    if (node && node.name !== clean) {
      await this.adapter.rename(id, clean);
      this.toast('Renamed');
    }
    await this.refresh();
  }

  cancelRename() {
    this.renamingId = null;
  }

  targets() {
    // selection if the focused node is selected, else the focus alone
    if (this.selected.size) return [...this.selected];
    return this.focusId ? [this.focusId] : [];
  }

  async trashSelection() {
    const ids = this.targets();
    if (!ids.length) return;
    for (const id of ids) await this.adapter.trash(id);
    this.clearSelection();
    this.menu = null;
    await this.refresh();
    const where = isHostId(ids[0]) ? this.binName : 'Trash';
    this.toast(ids.length > 1 ? `${ids.length} moved to ${where}` : `Moved to ${where}`);
  }

  async restoreSelection() {
    const ids = this.targets();
    if (!ids.length) return;
    for (const id of ids) await this.adapter.restore(id);
    this.clearSelection();
    this.menu = null;
    await this.refresh();
    this.toast('Restored');
  }

  async deleteForeverSelection() {
    const ids = this.targets();
    if (!ids.length) return;
    const question = ids.length > 1 ? `Delete ${ids.length} items for good? This cannot be undone.` : 'Delete this for good? This cannot be undone.';
    if (devicePrefs.get('storage.confirmDelete') && !(await ask(question, { yes: 'Delete forever', danger: true }))) return;
    for (const id of ids) await this.adapter.deleteForever(id);
    this.clearSelection();
    this.menu = null;
    await this.refresh();
    this.toast('Deleted forever');
  }

  async toggleStar(id = this.focusId) {
    if (!id) return;
    const node = await this.adapter.toggleStar(id);
    this.menu = null;
    await this.refresh();
    if (node) this.toast(node.starred ? 'Starred' : 'Unstarred');
  }

  async moveSelection(targetFolderId) {
    const ids = this.targets();
    if (!ids.length) return;
    for (const id of ids) await this.adapter.move(id, targetFolderId);
    this.clearSelection();
    await this.refresh();
    this.toast('Moved');
  }

  async uploadFiles(fileList) {
    const files = [...fileList];
    if (!files.length) return;
    if (this.atComputerTop) return this.toast('Open a drive first');
    const made = await this.adapter.upload(files, this.cwd.id);
    await this.refresh();
    this.toast(made.length > 1 ? `${made.length} uploaded` : 'Uploaded');
    if (made.length === 1) this.select(made[0].id);
  }

  download(id = this.focusId) {
    const node = this.items.find((i) => i.id === id);
    if (!node || node.kind === 'folder') return;
    const ok = this.adapter.download(node);
    this.menu = null;
    this.toast(ok ? 'Downloading' : 'Demo file — no bytes yet');
  }

  // ---------- overlays ----------
  openMenu(x, y, id) {
    this.menu = { x, y, id, key: uid() };
    if (id && !this.selected.has(id)) this.select(id);
  }

  closeMenu() {
    this.menu = null;
  }

  closePreview() {
    this.previewId = null;
  }

  /** Something of the drive's own is open over the files: Connect, the menu, a preview, a rename, New. */
  get hasOverlay() {
    return !!(this.connectOpen || this.menu || this.previewId || this.renamingId || this.newMenuOpen);
  }

  /** Esc priority: menu → preview → rename → new-menu → selection.
      Returns false when there was nothing to close or clear. */
  escape() {
    if (this.connectOpen) { this.connectOpen = false; return true; }
    if (this.menu) { this.closeMenu(); return true; }
    if (this.previewId) { this.closePreview(); return true; }
    if (this.renamingId) { this.cancelRename(); return true; }
    if (this.newMenuOpen) { this.newMenuOpen = false; return true; }
    if (!this.selected.size && !this.focusId) return false;
    this.clearSelection();
    return true;
  }

  toast(msg) {
    // A refused call toasts by itself and its caller often toasts the error again: once is enough.
    if (msg === REFUSED && this.toasts.some((t) => t.msg === msg)) return;
    const id = uid();
    this.toasts = [...this.toasts, { id, msg }];
    setTimeout(() => {
      this.toasts = this.toasts.filter((t) => t.id !== id);
    }, 2600);
  }

  async setQuery(q) {
    this.query = q;
    await this.refresh();
  }

  toggleViewMode() {
    this.setViewMode(this.viewMode === 'grid' ? 'list' : 'grid');
  }

  get previewNode() {
    return this.items.find((i) => i.id === this.previewId) || null;
  }
}

for (const name of GUARDED_ACTIONS) {
  const action = DriveStore.prototype[name];
  DriveStore.prototype[name] = async function (...args) {
    try {
      return await action.apply(this, args);
    } catch (err) {
      this.toast(err?.message || GENERIC_ERROR);
      try {
        await this.refresh();
      } catch {
        this.items = []; // the box is unreachable: show nothing, not stale files
      }
    }
  };
}

if (SHOWCASE) {
  for (const name of SHOWCASE_REFUSED) {
    DriveStore.prototype[name] = async function () {
      refused();
      return false;
    };
  }
}

export const store = new DriveStore();
onRefused((msg) => store.toast(msg));
