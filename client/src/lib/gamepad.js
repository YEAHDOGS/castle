// Game controllers. Reads every connected pad once a frame (only while one is connected)
// and turns its buttons into the keys the rest of Castle already understands, sent to the
// focused element as keydown events marked `fromPad`. See input.svelte.js for the map.
//
// It reads nothing while Castle's window is not the one in front: a game started from
// Arcade gets the controller to itself.

const STANDARD = {
  A: 0,
  B: 1,
  X: 2,
  Y: 3,
  LB: 4,
  RB: 5,
  LT: 6,
  RT: 7,
  VIEW: 8,
  START: 9,
  UP: 12,
  DOWN: 13,
  LEFT: 14,
  RIGHT: 15,
};

// Button -> the key it sends. The arrows repeat while held; the rest fire once per press.
const BUTTON_KEYS = [
  [STANDARD.A, { key: 'Enter' }],
  [STANDARD.B, { key: 'Escape' }],
  [STANDARD.X, { key: 'PadX' }],
  [STANDARD.Y, { key: 'PadY' }],
  [STANDARD.LB, { key: 'PadPrev' }],
  [STANDARD.RB, { key: 'PadNext' }],
  [STANDARD.LT, { key: 'PageUp' }],
  [STANDARD.RT, { key: 'PageDown' }],
  [STANDARD.VIEW, { key: 'PadView' }],
  [STANDARD.START, { key: 'Enter', shiftKey: true }],
];
const ARROWS = [
  [STANDARD.UP, 'ArrowUp'],
  [STANDARD.DOWN, 'ArrowDown'],
  [STANDARD.LEFT, 'ArrowLeft'],
  [STANDARD.RIGHT, 'ArrowRight'],
];

const PRESSED = 0.5; // an analogue trigger counts as pressed past this
const STICK_ON = 0.55; // the left stick counts as an arrow past this...
const STICK_OFF = 0.35; // ...and lets go below this, so it does not flicker at the edge
const SCROLL_DEADZONE = 0.18;
const SCROLL_SPEED = 22; // pixels per frame at full tilt of the right stick
const REPEAT_DELAY_MS = 380;
const REPEAT_EVERY_MS = 85;
const SONY = /054c|playstation|dualsense|dualshock|wireless controller/i;

let running = false;
let hooks = null;
const held = new Map(); // `${pad}:${name}` -> { since, last } for repeating arrows
const was = new Map(); // `${pad}:${button}` -> pressed last frame
let stickDir = new Map(); // pad index -> the arrow the left stick is holding, or null

function send(init) {
  const target = document.activeElement || document.body;
  const event = new KeyboardEvent('keydown', { bubbles: true, cancelable: true, composed: true, ...init });
  event.fromPad = true;
  const taken = !target.dispatchEvent(event);
  // A on something no handler took: press it, as Enter on a button would.
  if (!taken && init.key === 'Enter' && !init.shiftKey) hooks.accept(document.activeElement);
}

function pressed(button) {
  return !!button && (button.pressed || button.value > PRESSED);
}

/** Fires an arrow when it goes down, then again and again while it stays down. */
function arrow(id, key, down, now) {
  const state = held.get(id);
  if (!down) {
    held.delete(id);
    return;
  }
  if (!state) {
    held.set(id, { since: now, last: now });
    send({ key });
  } else if (now - state.since > REPEAT_DELAY_MS && now - state.last > REPEAT_EVERY_MS) {
    state.last = now;
    send({ key });
  }
}

function stickArrow(index, x, y) {
  const holding = stickDir.get(index) || null;
  const reach = Math.max(Math.abs(x), Math.abs(y));
  if (holding && reach < STICK_OFF) return null;
  if (!holding && reach < STICK_ON) return null;
  if (Math.abs(x) > Math.abs(y)) return x > 0 ? 'ArrowRight' : 'ArrowLeft';
  return y > 0 ? 'ArrowDown' : 'ArrowUp';
}

function frame() {
  const pads = navigator.getGamepads ? [...navigator.getGamepads()].filter(Boolean) : [];
  if (!pads.length) {
    running = false;
    held.clear();
    was.clear();
    stickDir = new Map();
    return;
  }
  requestAnimationFrame(frame);
  if (!document.hasFocus() || document.hidden) {
    held.clear(); // nothing carries over from while a game had the controller
    for (const pad of pads) for (let b = 0; b < pad.buttons.length; b++) was.set(`${pad.index}:${b}`, pressed(pad.buttons[b]));
    return;
  }
  const now = performance.now();
  for (const pad of pads) {
    let used = false;
    for (const [button, init] of BUTTON_KEYS) {
      const id = `${pad.index}:${button}`;
      const down = pressed(pad.buttons[button]);
      if (down && !was.get(id)) {
        used = true;
        hooks.onUse(SONY.test(pad.id) ? 'playstation' : 'xbox');
        send(init);
      }
      was.set(id, down);
    }
    for (const [button, key] of ARROWS) {
      const down = pressed(pad.buttons[button]);
      if (down) used = true;
      arrow(`${pad.index}:${key}`, key, down, now);
    }
    const [lx = 0, ly = 0, rx = 0, ry = 0] = pad.axes;
    const fromStick = stickArrow(pad.index, lx, ly);
    if (fromStick !== stickDir.get(pad.index)) {
      if (stickDir.get(pad.index)) held.delete(`${pad.index}:stick`);
      stickDir.set(pad.index, fromStick);
    }
    if (fromStick) used = true;
    arrow(`${pad.index}:stick`, fromStick, !!fromStick, now);
    if (Math.abs(rx) > SCROLL_DEADZONE || Math.abs(ry) > SCROLL_DEADZONE) {
      used = true;
      const ease = (v) => (Math.abs(v) > SCROLL_DEADZONE ? Math.sign(v) * (Math.abs(v) - SCROLL_DEADZONE) / (1 - SCROLL_DEADZONE) : 0);
      hooks.scrollBy(ease(rx) * SCROLL_SPEED, ease(ry) * SCROLL_SPEED);
    }
    if (used) hooks.onUse(SONY.test(pad.id) ? 'playstation' : 'xbox');
  }
}

function start() {
  if (running) return;
  running = true;
  requestAnimationFrame(frame);
}

/**
 * Starts listening for controllers.
 * @param {{ onUse: (kind: string) => void, accept: (el: Element) => void, scrollBy: (dx: number, dy: number) => void }} with
 */
export function startGamepads(withHooks) {
  if (typeof navigator === 'undefined' || !navigator.getGamepads) return;
  hooks = withHooks;
  window.addEventListener('gamepadconnected', start);
  // A pad already connected before the page loaded shows up on its first press.
  if ([...navigator.getGamepads()].some(Boolean)) start();
}
