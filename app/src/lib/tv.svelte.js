// TV remote mode: Castle driven by a pointer that is slow and clumsy, such as the cursor a
// smart TV's remote moves in its browser.
//
// While it is on, the pointer stops being a pointer. A nudge in any direction moves focus
// one step that way, as an arrow key would (input.svelte.js does the moving), and a click
// anywhere presses what is focused, as Enter would. Keeping the pointer moving the same
// way repeats the step, slowly, like a held key. The TV's Back button closes what is open,
// or goes Home when nothing is, instead of leaving Castle.
//
// A real mouse covers far more ground than a remote does. When the pointer moves like one,
// the mode switches itself off and says so, so nothing goes flying across the screen.
//
// It is turned on from the TV remote tile on Home, or by opening Castle with ?tv=1 (which
// is where castle.dogs.red/d sends a TV). ?tv=0 turns it off. The choice is kept on the
// device.
//
// The numbers below are first guesses, to be tuned on a real TV. `tv.last` holds the last
// thing the remote sent, and the strip across the top (TvStrip.svelte) shows it.

const SAVED_KEY = 'castle-tv';
const PARAM = 'tv';
const ON = '1';
const OFF = '0';

const NUDGE_PX = 3; // travel one way that counts as a nudge
const REST_MS = 220; // stillness that ends a nudge, so the next movement is a new one
const REPEAT_DELAY_MS = 650; // moving the same way this long starts repeating the step
const REPEAT_EVERY_MS = 320;
// A mouse: this much ground inside this long. A remote's cursor crawls; a hand does not.
const MOUSE_PX = 900;
const MOUSE_WINDOW_MS = 350;

const HAS_POINTER_EVENTS = typeof window !== 'undefined' && 'PointerEvent' in window;
const ARROW = { up: 'ArrowUp', down: 'ArrowDown', left: 'ArrowLeft', right: 'ArrowRight' };
const SIGN = { up: '↑', down: '↓', left: '←', right: '→' };

class Tv {
  on = $state(false);
  /** The last thing the remote did, in a few words, for the strip. */
  last = $state('');
}

export const tv = new Tv();

let hooks = null;
let nudge = null; // { dir, since, stepped, x, y, at }: the movement in progress
let lastPoint = null; // { x, y }: where the pointer was, for browsers with no movementX
let trail = []; // { t, d }: recent travel, to tell a mouse from a remote
let guarded = false; // whether a history entry stands between Back and leaving Castle

function save(on) {
  try {
    localStorage.setItem(SAVED_KEY, on ? ON : OFF);
  } catch {
    /* storage blocked: the choice lasts for this visit only */
  }
}

function readStart() {
  const asked = new URLSearchParams(location.search).get(PARAM);
  if (asked === ON || asked === OFF) {
    save(asked === ON);
    return asked === ON;
  }
  try {
    return localStorage.getItem(SAVED_KEY) === ON;
  } catch {
    return false;
  }
}

/** Turns the mode on or off. `why` is said in a toast when it is given. */
export function setTv(on, why = '') {
  if (tv.on === on) return;
  tv.on = on;
  save(on);
  nudge = null;
  lastPoint = null;
  trail = [];
  document.documentElement.dataset.tv = on ? 'on' : 'off';
  if (on) {
    guardBack();
    hooks?.keys();
    tv.last = 'waiting for the remote';
  }
  if (why) hooks?.say(why);
}

/** Sends an arrow to what is focused, the way a controller's D-pad does. */
function step(dir) {
  hooks.keys();
  const target = document.activeElement || document.body;
  const event = new KeyboardEvent('keydown', { key: ARROW[dir], bubbles: true, cancelable: true, composed: true });
  event.fromPad = true;
  target.dispatchEvent(event);
}

function directionOf(x, y) {
  if (Math.abs(x) >= Math.abs(y)) return x > 0 ? 'right' : 'left';
  return y > 0 ? 'down' : 'up';
}

/** True when the pointer has just covered more ground than a remote's cursor could. */
function movesLikeAMouse(now, travelled) {
  trail.push({ t: now, d: travelled });
  while (trail.length && now - trail[0].t > MOUSE_WINDOW_MS) trail.shift();
  let total = 0;
  for (const leg of trail) total += leg.d;
  return total > MOUSE_PX;
}

function onMove(e) {
  if (!tv.on || !e.isTrusted) return;
  // Where pointer events exist, each movement also arrives as a mouse event: count it once.
  if (e.type === 'mousemove' && HAS_POINTER_EVENTS) {
    e.stopPropagation();
    return;
  }
  let dx = e.movementX;
  let dy = e.movementY;
  // Older browsers leave movementX out: work it out from where the pointer was.
  if (typeof dx !== 'number' || typeof dy !== 'number' || (!dx && !dy && lastPoint)) {
    dx = lastPoint ? e.clientX - lastPoint.x : 0;
    dy = lastPoint ? e.clientY - lastPoint.y : 0;
  }
  lastPoint = { x: e.clientX, y: e.clientY };
  if (!dx && !dy) return;
  const now = performance.now();
  if (movesLikeAMouse(now, Math.hypot(dx, dy))) {
    setTv(false, 'TV remote off: that moved like a mouse');
    return;
  }
  // The page must not treat this as a mouse: no hover, no switching back to pointer mode.
  e.stopPropagation();

  if (nudge && now - nudge.at > REST_MS) nudge = null;
  if (!nudge) nudge = { dir: null, since: now, stepped: 0, x: 0, y: 0, at: now };
  nudge.at = now;
  nudge.x += dx;
  nudge.y += dy;
  if (Math.max(Math.abs(nudge.x), Math.abs(nudge.y)) < NUDGE_PX) return;

  const dir = directionOf(nudge.x, nudge.y);
  if (dir !== nudge.dir) {
    // A first movement, or a turn: one step straight away.
    nudge = { dir, since: now, stepped: now, x: 0, y: 0, at: now };
    tv.last = `nudge ${SIGN[dir]}`;
    step(dir);
    return;
  }
  nudge.x = 0;
  nudge.y = 0;
  if (now - nudge.since > REPEAT_DELAY_MS && now - nudge.stepped > REPEAT_EVERY_MS) {
    nudge.stepped = now;
    tv.last = `hold ${SIGN[dir]}`;
    step(dir);
  }
}

/** A click anywhere presses what is focused: the cursor is not where the person is looking. */
function onClick(e) {
  if (!tv.on || !e.isTrusted) return;
  e.preventDefault();
  e.stopPropagation();
  if (e.type !== 'click') return; // the press and release that led to it are only swallowed
  tv.last = 'press';
  hooks.keys();
  hooks.accept(document.activeElement);
}

function onKey(e) {
  if (!tv.on || !e.isTrusted) return;
  tv.last = `key ${e.key || '?'} (${e.keyCode})`;
}

/** Puts one history entry in front of the page, so the TV's Back lands here first. */
function guardBack() {
  if (guarded) return;
  guarded = true;
  history.pushState({ castleTv: true }, '');
}

function onBack() {
  guarded = false;
  if (!tv.on) return;
  guardBack();
  tv.last = 'back';
  const target = document.activeElement || document.body;
  const event = new KeyboardEvent('keydown', { key: 'Escape', bubbles: true, cancelable: true, composed: true });
  event.fromPad = true;
  // With nothing open to close, Back goes Home, as it would in a TV app.
  if (target.dispatchEvent(event) && !hooks.modalOpen()) hooks.home();
}

/**
 * Starts TV remote mode's listeners. They do nothing until the mode is on.
 * @param {{ keys: () => void, accept: (el: Element) => void, say: (msg: string) => void, modalOpen: () => boolean, home: () => void }} withHooks
 *   keys: show focus rings, as when the keyboard is in use. accept: press what is focused.
 */
export function installTv(withHooks) {
  if (typeof window === 'undefined' || hooks) return;
  hooks = withHooks;
  // Capture, on the window: before the page, and before input.svelte.js's own listeners.
  for (const type of ['pointermove', 'mousemove']) window.addEventListener(type, onMove, true);
  for (const type of ['click', 'pointerdown', 'pointerup', 'mousedown', 'mouseup']) window.addEventListener(type, onClick, true);
  window.addEventListener('keydown', onKey, true);
  window.addEventListener('popstate', onBack);
  if (readStart()) setTv(true);
}
