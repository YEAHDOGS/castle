// Input: Castle without a mouse. The keyboard and a game controller both drive it.
//
// One idea runs through it: the arrows move focus to whatever sits that way on screen
// (spatial navigation), Enter or A presses what is focused, Escape or B goes back.
// Each section can take a key first, because its handlers listen on the document (or on
// elements) and this layer listens on the window, after them. It only acts on a key
// nobody else took (defaultPrevented tells it).
//
//  Shift+Enter (controller: Start)  … opens or closes the section dial, from anywhere
//  Arrows (controller: D-pad, left stick) … move focus that way
//  Enter (A)                        … press what is focused
//  Escape (B)                       … back: closes the dialog on top
//  PageUp / PageDown (LT / RT)      … a screen up or down, focus following
//  LB / RB                          … previous / next view in the section's rail
//  View (the button left of Start)  … Home
//  Right stick                      … scrolls
//
// A controller's buttons arrive as keydown events on the focused element (see gamepad.js),
// so every handler written for the keyboard works for the controller too. The buttons a
// keyboard does not have come as the PAD keys below, and carry `fromPad`.
//
// It also keeps focus somewhere sensible: into a dialog when one opens, back to what
// opened it when it closes, and onto the nearest thing when the focused one disappears.

import { tick, untrack } from 'svelte';
import { shell } from './shell.svelte.js';
import { startGamepads } from './gamepad.js';

/** Controller buttons with no key of their own. */
export const PAD = {
  X: 'PadX',
  Y: 'PadY',
  PREV: 'PadPrev', // LB
  NEXT: 'PadNext', // RB
  VIEW: 'PadView',
};

const FOCUSABLE = 'a[href], button, input:not([type="hidden"]), select, textarea, summary, [tabindex], [contenteditable="true"]';
const SKIP = '[inert], [aria-hidden="true"], [data-nav-skip]';
const MODAL = '[aria-modal="true"]';
const CLOSER = '[data-nav-back], [aria-label^="Close" i], [aria-label^="Stop and close" i], [aria-label^="Cancel" i]';
const TEXT_TYPES = new Set(['text', 'search', 'email', 'url', 'tel', 'password', 'number', '']);
const PAGE_SHARE = 0.85; // how much of a screen PageUp / PageDown move
const CROSS_WEIGHT = 2; // how much being out of line costs against being far away
const ALIGN_WEIGHT = 0.1; // breaks ties toward the one most in line
const DIRS = { ArrowUp: 'up', ArrowDown: 'down', ArrowLeft: 'left', ArrowRight: 'right' };

// ---------------------------------------------------------------- state

class Input {
  /** What the person last used: 'pointer', 'keys' or 'pad'. Rings and hints follow it. */
  mode = $state('pointer');
  /** The kind of controller, for the button names in the hints: 'xbox' or 'playstation'. */
  padKind = $state('xbox');
  /** Button hints by owner; the highest level shows. See setHints. */
  hintSets = $state([]);
  /** True while something sits along the bottom of the screen (the music bar). */
  lift = $state(false);

  get hints() {
    let best = null;
    for (const set of this.hintSets) if (!best || set.level >= best.level) best = set;
    return best?.list || null;
  }
}

export const input = new Input();

/**
 * Shows controller hints while the caller is on screen. Higher levels win: a section is 1,
 * a card over it 2, a player over that 3. Returns the function that takes them down, so
 * `$effect(() => setHints(...))` keeps them in step.
 * @param {{ btn: string, label: string }[]} list btn: A B X Y LB RB LT RT START VIEW DPAD
 */
export function setHints(owner, list, level = 1) {
  const entry = { owner, list, level };
  // Untracked: an effect that calls this must not re-run because the list it writes changed.
  untrack(() => {
    input.hintSets = [...input.hintSets.filter((s) => s.owner !== owner), entry];
  });
  return () => {
    untrack(() => {
      input.hintSets = input.hintSets.filter((s) => s !== entry);
    });
  };
}

function setMode(next) {
  if (input.mode === next) return;
  input.mode = next;
  document.documentElement.dataset.input = next;
}

// ---------------------------------------------------------------- what can take focus

const rectOf = (el) => el.getBoundingClientRect();
const centre = (r) => ({ x: r.left + r.width / 2, y: r.top + r.height / 2 });

function isCandidate(el) {
  if (el.disabled || el.tabIndex < 0) return false;
  if (el.closest(SKIP)) return false;
  return el.getClientRects().length > 0;
}

/** A dialog on its way out: Svelte marks what is leaving inert while it fades. */
const closing = (m) => m.inert || !!m.closest('[inert]') || !!m.querySelector(':scope > [inert]');

/** Every open dialog, in page order; the last is on top. */
function openModals() {
  return [...document.querySelectorAll(MODAL)].filter((m) => !closing(m) && m.getClientRects().length > 0);
}

/** Where focus may go now: the dialog on top, or the whole page when none is open. */
function scope() {
  return openModals().at(-1) || document.body;
}

function candidates(root = scope()) {
  const list = [...root.querySelectorAll(FOCUSABLE)].filter(isCandidate);
  // With no dialog open, nothing inside a closed-but-fading one counts either.
  return root === document.body ? list.filter((el) => !el.closest(MODAL)) : list;
}

/** Where focus starts in a scope: what it marks with data-autofocus, else the first game or
    poster (.nav-card), else the first thing in its main area. */
function startIn(root = scope()) {
  const marked = [...root.querySelectorAll('[data-autofocus]')].find((el) => isCandidate(el) && (root !== document.body || !el.closest(MODAL)));
  if (marked) return marked;
  const main = root.querySelector('main');
  const inMain = main ? candidates(main) : [];
  return inMain.find((el) => el.classList.contains('nav-card')) || inMain[0] || candidates(root)[0] || null;
}

function reducedMotion() {
  return document.documentElement.dataset.motion === 'reduced' || matchMedia('(prefers-reduced-motion: reduce)').matches;
}

/** Focuses an element and, unless the mouse is in charge, scrolls it into view. */
export function focusEl(el, { scroll = input.mode !== 'pointer' } = {}) {
  if (!el) return false;
  el.focus({ preventScroll: true });
  if (scroll) el.scrollIntoView({ block: 'nearest', inline: 'nearest', behavior: reducedMotion() ? 'auto' : 'smooth' });
  return document.activeElement === el;
}

/** Focuses the first match once the page has drawn: for putting focus back after a view changes. */
export async function focusSoon(selector) {
  await tick();
  requestAnimationFrame(() => {
    const el = typeof selector === 'string' ? document.querySelector(selector) : selector;
    if (el && isCandidate(el)) focusEl(el);
  });
}

/** True when a dialog is open over the page. */
export function modalOpen() {
  return openModals().length > 0;
}

// ---------------------------------------------------------------- moving focus

/** How far `to` is from `from` going `dir`, or null when it is not that way at all. */
function distance(dir, from, to) {
  const a = centre(from);
  const b = centre(to);
  let main;
  let cross;
  if (dir === 'right' || dir === 'left') {
    const ahead = dir === 'right' ? to.left >= from.right - 1 : to.right <= from.left + 1;
    const past = dir === 'right' ? b.x > a.x + 1 : b.x < a.x - 1;
    if (!ahead && !past) return null;
    main = dir === 'right' ? Math.max(0, to.left - from.right) : Math.max(0, from.left - to.right);
    cross = Math.max(0, to.top - from.bottom, from.top - to.bottom);
    if (!ahead) main += Math.abs(b.x - a.x); // overlapping: only when nothing cleaner is there
    return main + CROSS_WEIGHT * cross + ALIGN_WEIGHT * Math.abs(b.y - a.y);
  }
  const ahead = dir === 'down' ? to.top >= from.bottom - 1 : to.bottom <= from.top + 1;
  const past = dir === 'down' ? b.y > a.y + 1 : b.y < a.y - 1;
  if (!ahead && !past) return null;
  main = dir === 'down' ? Math.max(0, to.top - from.bottom) : Math.max(0, from.top - to.bottom);
  cross = Math.max(0, to.left - from.right, from.left - to.right);
  if (!ahead) main += Math.abs(b.y - a.y);
  return main + CROSS_WEIGHT * cross + ALIGN_WEIGHT * Math.abs(b.x - a.x);
}

/** Moves focus to the nearest thing `dir` of the focused one. Returns whether it moved. */
export function move(dir) {
  const root = scope();
  const current = document.activeElement;
  if (!current || current === document.body || !root.contains(current) || !isCandidate(current)) {
    return focusEl(startIn(root));
  }
  const from = rectOf(current);
  let best = null;
  let bestScore = Infinity;
  for (const el of candidates(root)) {
    if (el === current || el.contains(current) || current.contains(el)) continue;
    const score = distance(dir, from, rectOf(el));
    if (score !== null && score < bestScore) {
      best = el;
      bestScore = score;
    }
  }
  return best ? focusEl(best) : false;
}

function scrollParent(el) {
  for (let node = el?.parentElement; node && node !== document.body; node = node.parentElement) {
    const { overflowY } = getComputedStyle(node);
    if ((overflowY === 'auto' || overflowY === 'scroll') && node.scrollHeight > node.clientHeight) return node;
  }
  return null;
}

function scrollBoxFor(el) {
  const root = scope();
  return scrollParent(el && el !== document.body ? el : null) || scrollParent(root.querySelector('main *')) || root.querySelector('main') || null;
}

/** A screen up or down, then focus on whatever is now nearest where the focus was. */
export function page(dir) {
  const current = document.activeElement !== document.body ? document.activeElement : null;
  const box = scrollBoxFor(current);
  if (!box) return false;
  const was = current ? centre(rectOf(current)) : null;
  box.scrollBy({ top: dir * box.clientHeight * PAGE_SHARE, behavior: 'auto' });
  const view = rectOf(box);
  const inView = candidates(scope()).filter((el) => {
    if (!box.contains(el)) return false;
    const r = rectOf(el);
    return r.top >= view.top && r.bottom <= view.bottom;
  });
  if (!inView.length) return true;
  const aim = was || { x: view.left, y: view.top };
  let best = inView[0];
  let bestGap = Infinity;
  for (const el of inView) {
    const c = centre(rectOf(el));
    const gap = Math.hypot(c.x - aim.x, c.y - aim.y);
    if (gap < bestGap) {
      best = el;
      bestGap = gap;
    }
  }
  best.focus({ preventScroll: true });
  return true;
}

/** Scrolls whatever holds the focus (the right stick). */
export function scrollBy(dx, dy) {
  const box = scrollBoxFor(document.activeElement);
  (box || document.scrollingElement)?.scrollBy({ left: dx, top: dy, behavior: 'auto' });
}

// ---------------------------------------------------------------- pressing and going back

const isTextField = (el) =>
  el?.tagName === 'TEXTAREA' || el?.isContentEditable || (el?.tagName === 'INPUT' && TEXT_TYPES.has(el.type));

/** Moves a dropdown one option along, as a controller cannot open the browser's own list. */
function stepSelect(el, by) {
  const options = [...el.options].filter((o) => !o.disabled);
  const at = options.findIndex((o) => o.value === el.value);
  const next = options[(at + by + options.length) % options.length];
  if (!next || next.value === el.value) return;
  el.value = next.value;
  el.dispatchEvent(new Event('input', { bubbles: true }));
  el.dispatchEvent(new Event('change', { bubbles: true }));
}

function stepRange(el, by) {
  if (by > 0) el.stepUp();
  else el.stepDown();
  el.dispatchEvent(new Event('input', { bubbles: true }));
  el.dispatchEvent(new Event('change', { bubbles: true }));
}

/** The controller's A on what is focused, when no handler took the Enter it sent first. */
export function accept(el) {
  if (!el || el === document.body || !scope().contains(el)) return focusEl(startIn());
  if (el.tagName === 'SELECT') return stepSelect(el, 1);
  if (isTextField(el) || el.tagName === 'INPUT' && el.type === 'range') return;
  el.click();
}

/** Escape or B that nobody took: closes the dialog on top through its own close button. */
function back() {
  const top = openModals().at(-1);
  if (!top) return false;
  const closer = top.querySelector(CLOSER);
  if (!closer) return false;
  closer.click();
  return true;
}

// ---------------------------------------------------------------- the keys nobody else took

/** Whether an element keeps an arrow for itself (a caret to move, a value to change). */
function ownsArrow(el, dir, e) {
  if (!el || el === document.body) return false;
  if (el.tagName === 'TEXTAREA' || el.isContentEditable) return true;
  if (el.tagName === 'SELECT') return dir === 'up' || dir === 'down';
  if (el.tagName === 'INPUT') {
    if (el.type === 'range') return true;
    if (el.type === 'number') return dir === 'up' || dir === 'down'; // up and down count
    if (TEXT_TYPES.has(el.type)) {
      if (dir === 'up' || dir === 'down') return false;
      if (el.selectionStart !== el.selectionEnd || e.shiftKey) return true;
      return dir === 'left' ? el.selectionStart > 0 : el.selectionEnd < el.value.length;
    }
  }
  if (el.tagName === 'VIDEO' || el.tagName === 'AUDIO') return true;
  return false;
}

function onKey(e) {
  if (e.defaultPrevented || e.ctrlKey || e.metaKey || e.altKey) return;
  const el = document.activeElement;
  const pad = !!e.fromPad;
  const dir = DIRS[e.key];
  if (dir) {
    if (pad && el?.tagName === 'SELECT' && (dir === 'left' || dir === 'right')) {
      stepSelect(el, dir === 'right' ? 1 : -1);
    } else if (pad && el?.tagName === 'INPUT' && el.type === 'range' && (dir === 'left' || dir === 'right')) {
      stepRange(el, dir === 'right' ? 1 : -1);
    } else if (!pad && ownsArrow(el, dir, e)) {
      return;
    } else {
      move(dir);
    }
    e.preventDefault();
    return;
  }
  switch (e.key) {
    case 'PageUp':
    case 'PageDown':
      if (!pad && isTextField(el)) return;
      if (page(e.key === 'PageDown' ? 1 : -1)) e.preventDefault();
      return;
    case 'Escape':
      if (back()) e.preventDefault();
      return;
    case PAD.PREV:
    case PAD.NEXT:
      if (!modalOpen()) window.dispatchEvent(new CustomEvent('castle-view', { detail: e.key === PAD.NEXT ? 1 : -1 }));
      return;
    case PAD.VIEW:
      shell.goHome();
      return;
  }
}

/** Shift+Enter opens and closes the dial before anything else sees the key. */
function onKeyFirst(e) {
  if (e.key !== 'Enter' || !e.shiftKey || e.ctrlKey || e.metaKey || e.altKey) return;
  const t = e.target;
  if (t?.tagName === 'TEXTAREA' || t?.isContentEditable) return; // a new line there
  e.preventDefault();
  e.stopPropagation();
  shell.wheelOpen = !shell.wheelOpen;
}

// ---------------------------------------------------------------- keeping focus somewhere

let lastTop = null;
let lastModals = [];
let lastFocusRect = null;
const returnTo = new WeakMap(); // dialog -> what was focused before it opened
let checkQueued = false;

function nearestTo(rect, root) {
  if (!rect) return null;
  const aim = centre(rect);
  let best = null;
  let bestGap = Infinity;
  for (const el of candidates(root)) {
    const c = centre(rectOf(el));
    const gap = Math.hypot(c.x - aim.x, c.y - aim.y);
    if (gap < bestGap) {
      best = el;
      bestGap = gap;
    }
  }
  return best;
}

function checkFocus() {
  checkQueued = false;
  const modals = openModals();
  const top = modals.at(-1) || null;
  const root = top || document.body;
  const active = document.activeElement;
  const lost = !active || active === document.body || !root.contains(active) || (!top && active.closest(MODAL));

  if (top !== lastTop) {
    const opened = top && !lastModals.includes(top);
    if (opened) {
      if (active && active !== document.body && !top.contains(active)) returnTo.set(top, active);
      // A dialog that manages its own focus (the dial) says so with data-nav-own.
      if (!top.hasAttribute('data-nav-own') && !top.contains(document.activeElement)) {
        focusEl(startIn(top), { scroll: false });
      }
    } else if (lastTop && lost) {
      // Back to what opened the dialog, unless something already took focus (a section
      // that put it on the game the card ended on, say).
      const back = returnTo.get(lastTop);
      if (back?.isConnected && root.contains(back) && isCandidate(back)) focusEl(back, { scroll: false });
      else if (input.mode !== 'pointer') focusEl(startIn(root));
    }
    lastTop = top;
    lastModals = modals;
    return;
  }
  lastModals = modals;
  // The focused thing went away (a card stepped, a view changed): take the nearest.
  if (lost && input.mode !== 'pointer') {
    focusEl([...root.querySelectorAll('[data-autofocus]')].find(isCandidate) || nearestTo(lastFocusRect, root) || startIn(root));
  }
}

function queueCheck() {
  if (checkQueued) return;
  checkQueued = true;
  requestAnimationFrame(checkFocus);
}

// ---------------------------------------------------------------- start

let installed = false;

export function installInput() {
  if (installed || typeof window === 'undefined') return;
  installed = true;
  document.documentElement.dataset.input = input.mode;

  window.addEventListener('keydown', onKeyFirst, true);
  window.addEventListener('keydown', onKey);
  // What was last used decides whether focus rings and hints show.
  window.addEventListener(
    'keydown',
    (e) => {
      if (e.isTrusted && !['Shift', 'Control', 'Alt', 'Meta'].includes(e.key)) setMode('keys');
    },
    true
  );
  window.addEventListener('pointerdown', () => setMode('pointer'), true);
  window.addEventListener(
    'pointermove',
    (e) => {
      if (e.movementX || e.movementY) setMode('pointer');
    },
    true
  );
  document.addEventListener('focusin', (e) => {
    if (e.target instanceof Element) lastFocusRect = rectOf(e.target);
  });
  new MutationObserver(queueCheck).observe(document.body, { childList: true, subtree: true, attributes: true, attributeFilter: ['inert', 'aria-modal'] });

  startGamepads({
    onUse(kind) {
      input.padKind = kind;
      setMode('pad');
    },
    accept,
    scrollBy,
  });
}
