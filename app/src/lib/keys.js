// Keyboard-first power UX. Every shortcut fires on keydown — zero perceptible lag.
//
//  Ctrl/Cmd+N or Ctrl/Cmd+Shift+N … new folder, instantly inline-renaming
//  F2                          … rename focused node inline
//  Enter                       … open (folder drills in, file previews)
//  Delete                      … move selection to Trash
//  Backspace                   … up one folder
//  Arrows (+Shift)             … move focus (extend selection)
//  Ctrl/Cmd+A                  … select all
//  Esc                         … menu → preview → rename → selection (priority order)
//  Shift+F10, the menu key       … the focused file's menu (a controller's X)
//
// handleDriveKey is shared: the FileBrowser container owns keys when focus is
// inside it ([data-kb]), the document listener owns them everywhere else. An arrow at the
// edge of the grid is left alone, so the app's spatial navigation (lib/input.svelte.js)
// carries focus out of the grid to whatever is that way.

import { store } from './store.svelte.js';
import { shell } from './shell.svelte.js';
import { PAD, modalOpen } from './input.svelte.js';

const INTERACTIVE = 'button, a[href], input, select, textarea, summary, [role="button"], [role="menuitem"], [role="tab"], [role="switch"]';

export function handleDriveKey(e) {
  const t = e.target;
  const typing =
    t && (t.tagName === 'INPUT' || t.tagName === 'TEXTAREA' || t.isContentEditable);

  // Escape always wins, even mid-rename.
  if (e.key === 'Escape') {
    // A controller's B is "back": out of whatever is open, else up one folder.
    if (e.fromPad && !store.hasOverlay) {
      e.preventDefault();
      store.goUp();
      return true;
    }
    if (!store.escape()) return false; // nothing here to close: let the app go back
    e.preventDefault();
    return true;
  }
  if (typing) return false; // rename input owns its keys (Enter commits there)

  const mod = e.ctrlKey || e.metaKey;

  if (mod && (e.key === 'n' || e.key === 'N')) {
    e.preventDefault();
    store.newFolder();
    return true;
  }
  if (mod && e.key.toLowerCase() === 'a') {
    e.preventDefault();
    store.selectAll();
    return true;
  }
  if (e.key === PAD.X || e.key === 'ContextMenu' || (e.shiftKey && e.key === 'F10')) {
    const el = store.focusId && document.querySelector(`[data-node="${store.focusId}"]`);
    if (!el) return false;
    e.preventDefault();
    const r = el.getBoundingClientRect();
    store.openMenu(r.left + 12, r.top + r.height / 2, store.focusId);
    return true;
  }
  switch (e.key) {
    case 'F2':
      e.preventDefault();
      store.startRename();
      return true;
    case 'Enter':
      e.preventDefault();
      store.activateFocus();
      return true;
    case 'Delete':
      e.preventDefault();
      store.trashSelection();
      return true;
    case 'Backspace':
      e.preventDefault();
      store.goUp();
      return true;
    case 'ArrowRight':
    case 'ArrowLeft':
    case 'ArrowUp':
    case 'ArrowDown':
      if (!store.moveFocus(e.key, e.shiftKey)) return false; // at the edge: focus leaves the grid
      e.preventDefault();
      return true;
  }
  // type-ahead: printable chars jump to the matching filename prefix
  if (e.key.length === 1 && e.key !== ' ' && !mod && !e.altKey) {
    store.jumpToPrefix(e.key);
    return true;
  }
  return false;
}

export function attachShortcuts() {
  const onKey = (e) => {
    if (e.defaultPrevented) return;
    const t = e.target;
    if (t?.closest?.('[data-kb]')) return; // the browser owns keys there
    if (shell.wheelOpen || shell.userMenuOpen) return; // those menus own the keyboard while open
    // A dialog that is not the drive's own (a question, Connect) keeps its keys.
    if (modalOpen() && !store.hasOverlay) return;
    // On a button, a field or a link, only Escape and the new-folder shortcut are the drive's:
    // the arrows move between controls and Enter presses the one focused.
    if (t !== document.body && t?.closest?.(INTERACTIVE)) {
      const mod = e.ctrlKey || e.metaKey;
      if (e.key !== 'Escape' && !(mod && e.key.toLowerCase() === 'n')) return;
    } else if (e.key in ARROWS || e.key === 'Enter') {
      return; // nothing focused yet: the app's navigation puts focus on the files first
    }
    handleDriveKey(e);
  };
  // On the document, so this runs before the app's navigation on the window.
  document.addEventListener('keydown', onKey);
  return () => document.removeEventListener('keydown', onKey);
}

const ARROWS = { ArrowUp: 1, ArrowDown: 1, ArrowLeft: 1, ArrowRight: 1 };
