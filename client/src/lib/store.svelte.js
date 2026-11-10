// DriveStore — all drive state + actions in one reactive class.
// The UI is a thin skin over this. Keyboard shortcuts call these directly,
// so everything here must feel instant: no awaits that block the first paint.

import { startingAdapter, localAdapter, fixedDemoAdapter, connectBox, forgetBox, takeConnectLink, isDesktopApp, localBox } from './box.js';
import { SvelteSet } from 'svelte/reactivity';
import { uid } from './format.js';
import { prefs as devicePrefs } from './prefs.svelte.js';

export const VIEWS = ['drive', 'shared', 'recent', 'starred', 'trash'];
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

  get cwd() {
    return this.path[this.path.length - 1];
  }

  get isTrash() {
    return this.view === 'trash';
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
    this.clearSelection();
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
    if (this.query.trim()) {
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

  // ---------- navigation ----------
  async setView(v) {
    if (this.view === v) return;
    this.view = v;
    this.slideDir = 1;
    if (v === 'drive') this.path = [{ id: 'root', name: 'My Drive' }];
    this.query = '';
    this.clearSelection();
    await this.refresh();
  }

  async openNode(node) {
    if (!node) return;
    if (node.kind === 'folder') {
      this.view = 'drive';
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
    if (this.view !== 'drive' || this.path.length <= 1 || this.query) return;
    this.slideDir = -1;
    this.path = this.path.slice(0, -1);
    this.clearSelection();
    await this.refresh();
  }

  async goToCrumb(i) {
    if (this.view !== 'drive') return;
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

  moveFocus(key, extend = false) {
    if (!this.items.length) return;
    const ids = this.items.map((i) => i.id);
    let idx = ids.indexOf(this.focusId);
    if (idx === -1) idx = 0;
    const cols = this.viewMode === 'grid' ? (this.columnsProvider?.() || 4) : 1;
    if (key === 'ArrowRight') idx = Math.min(idx + 1, ids.length - 1);
    else if (key === 'ArrowLeft') idx = Math.max(idx - 1, 0);
    else if (key === 'ArrowDown') idx = Math.min(idx + cols, ids.length - 1);
    else if (key === 'ArrowUp') idx = Math.max(idx - cols, 0);
    else return;
    const id = ids[idx];
    if (extend) {
      if (!this.selAnchor) this.selAnchor = this.focusId || ids[0];
      this.select(id, false, true);
    } else {
      this.select(id);
    }
    this.focusId = id;
    this.ensureVisible(id);
  }

  /** Auto-scroll the node into view and hand it DOM focus (roving tabindex). */
  ensureVisible(id) {
    requestAnimationFrame(() => {
      const el = document.querySelector(`[data-node="${id}"]`);
      el?.scrollIntoView({ block: 'nearest' });
      if (document.activeElement?.closest?.('[data-kb]')) el?.focus({ preventScroll: true });
    });
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
    this.toast(ids.length > 1 ? `${ids.length} moved to Trash` : 'Moved to Trash');
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
    if (devicePrefs.get('storage.confirmDelete') && !window.confirm(question)) return;
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

  /** Esc priority: menu → preview → rename → new-menu → selection. */
  escape() {
    if (this.connectOpen) { this.connectOpen = false; return; }
    if (this.menu) return this.closeMenu();
    if (this.previewId) return this.closePreview();
    if (this.renamingId) return this.cancelRename();
    if (this.newMenuOpen) { this.newMenuOpen = false; return; }
    this.clearSelection();
  }

  toast(msg) {
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

export const store = new DriveStore();
