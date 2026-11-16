// Where a bubble goes, given the box it points at. Pure arithmetic, so Blurb.svelte stays
// about drawing.
//
//   place(rect, size, side, at, bounds, taken)
//     rect    the target's getBoundingClientRect()
//     size    { w, h } of the bubble, once drawn
//     side    the side of the target the bubble prefers: 'top' | 'bottom' | 'left' | 'right'
//     at      where along the target's edge the tail lands, 0 to 1
//     bounds  { w, h } the room there is: the window, minus the showcase strip at the bottom
//     taken   [{ left, top, w, h }] where other bubbles already are
//   returns { left, top, side, tx, ty }: the bubble's corner, the side it ended up on, and
//   the tail's point inside the bubble.
//
// The preferred side is tried first; when the bubble would not fit there (a phone is narrow),
// or would cover another bubble, the other sides are tried in turn, and the first that fits
// clear of the others wins. Failing that, a bubble that fits but covers another is slid away
// from its target until it is clear (its tail then floats a little short of the target);
// failing that too, the first that fits at all. If none does, the preferred side is kept
// and the bubble is pushed inside the window.

const GAP = 16; // room between target and bubble: the tail lives here
const MARGIN = 8; // the bubble never comes closer than this to the window's edge
const TAIL_MIN = 18; // the tail stays this far from the bubble's corners
const OPPOSITE = { top: 'bottom', bottom: 'top', left: 'right', right: 'left' };

const clamp = (v, lo, hi) => Math.min(Math.max(v, lo), Math.max(lo, hi));

/** The point on the target's edge the tail comes from, on a given side. */
function anchor(rect, side, at) {
  if (side === 'top') return { x: rect.left + rect.width * at, y: rect.top };
  if (side === 'bottom') return { x: rect.left + rect.width * at, y: rect.bottom };
  if (side === 'left') return { x: rect.left, y: rect.top + rect.height * at };
  return { x: rect.right, y: rect.top + rect.height * at };
}

function attempt(rect, size, side, at, bounds) {
  const a = anchor(rect, side, at);
  let left;
  let top;
  if (side === 'top' || side === 'bottom') {
    // The tail sits a third of the way along the bubble, not dead centre: it reads as speech.
    left = clamp(a.x - size.w * 0.38, MARGIN, bounds.w - size.w - MARGIN);
    top = side === 'bottom' ? a.y + GAP : a.y - GAP - size.h;
  } else {
    left = side === 'right' ? a.x + GAP : a.x - GAP - size.w;
    top = clamp(a.y - size.h * 0.35, MARGIN, bounds.h - size.h - MARGIN);
  }
  const fits = left >= MARGIN && left + size.w <= bounds.w - MARGIN && top >= MARGIN && top + size.h <= bounds.h - MARGIN;
  return {
    left,
    top,
    side,
    fits,
    tx: clamp(a.x - left, TAIL_MIN, size.w - TAIL_MIN),
    ty: clamp(a.y - top, TAIL_MIN, size.h - TAIL_MIN),
  };
}

const overlaps = (a, b) => a.left < b.left + b.w && a.left + a.w > b.left && a.top < b.top + b.h && a.top + a.h > b.top;
const SLIDE_GAP = 18; // between a bubble slid aside and the one it was covering: room for its tail

/** The bubble slid away from its target, past every bubble it covers, or null when that leaves the window. */
function slid(p, size, bounds, taken) {
  const box = { left: p.left, top: p.top, w: size.w, h: size.h };
  for (let i = 0; i < taken.length; i++) {
    const hit = taken.find((t) => overlaps(box, t));
    if (!hit) break;
    if (p.side === 'top') box.top = hit.top - size.h - SLIDE_GAP;
    else if (p.side === 'bottom') box.top = hit.top + hit.h + SLIDE_GAP;
    else if (p.side === 'left') box.left = hit.left - size.w - SLIDE_GAP;
    else box.left = hit.left + hit.w + SLIDE_GAP;
  }
  if (box.left < MARGIN || box.left + size.w > bounds.w - MARGIN || box.top < MARGIN || box.top + size.h > bounds.h - MARGIN) return null;
  // The tail keeps pointing at the target, from whichever edge is nearest it now.
  return { ...p, left: box.left, top: box.top, tx: clamp(p.tx + p.left - box.left, TAIL_MIN, size.w - TAIL_MIN), ty: clamp(p.ty + p.top - box.top, TAIL_MIN, size.h - TAIL_MIN) };
}

export function place(rect, size, side = 'bottom', at = 0.5, bounds, taken = []) {
  const order = [side, OPPOSITE[side], 'bottom', 'top', 'left', 'right'].filter((s, i, all) => all.indexOf(s) === i);
  const tries = order.map((s) => attempt(rect, size, s, at, bounds));
  const clear = (p) => !taken.some((t) => overlaps({ left: p.left, top: p.top, w: size.w, h: size.h }, t));
  const fitting = tries.filter((p) => p.fits);
  const found = fitting.find(clear) || fitting.map((p) => slid(p, size, bounds, taken)).find((p) => p && clear(p)) || fitting[0];
  if (found) return found;
  // Nowhere fits (a very small window): keep the preferred side, inside the window at least.
  const first = tries[0];
  first.left = clamp(first.left, MARGIN, bounds.w - size.w - MARGIN);
  first.top = clamp(first.top, MARGIN, bounds.h - size.h - MARGIN);
  return first;
}

/** Whether a target is on screen enough to point at: some of it inside the bounds. */
export function onScreen(rect, bounds) {
  if (!rect || rect.width === 0 || rect.height === 0) return false;
  return rect.bottom > MARGIN && rect.top < bounds.h - MARGIN && rect.right > MARGIN && rect.left < bounds.w - MARGIN;
}
