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
//
// handleDriveKey is shared: the FileBrowser container owns keys when focus is
// inside it ([data-kb]), the window listener owns them everywhere else.

import { store } from './store.svelte.js';
import { shell } from './shell.svelte.js';

export function handleDriveKey(e) {
  const t = e.target;
  const typing =
    t && (t.tagName === 'INPUT' || t.tagName === 'TEXTAREA' || t.isContentEditable);

  // Escape always wins, even mid-rename.
  if (e.key === 'Escape') {
    e.preventDefault();
    store.escape();
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
      e.preventDefault();
      store.moveFocus(e.key, e.shiftKey);
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
    if (e.target?.closest?.('[data-kb]')) return; // the browser owns keys there
    if (shell.wheelOpen || shell.userMenuOpen) return; // those menus own the keyboard while open
    handleDriveKey(e);
  };
  window.addEventListener('keydown', onKey);
  return () => window.removeEventListener('keydown', onKey);
}
