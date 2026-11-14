// swipe: a Svelte action for flicking left and right with a finger.
//
//   <div use:swipe={(dir) => step(dir)}>   dir is 1 for a swipe to the left (the next one,
//                                           as on a phone's photo viewer), -1 for the right
//
// Only a mostly sideways flick counts, so scrolling up and down inside the element still
// works, and a tap is never mistaken for a swipe. Touch only: a mouse keeps its arrows.

const MIN_PX = 48; // how far the finger has to travel sideways
const SIDEWAYS = 1.4; // and how much more sideways than up or down

export function swipe(node, onswipe) {
  let handler = onswipe;
  let start = null;

  function down(e) {
    start = e.touches.length === 1 ? { x: e.touches[0].clientX, y: e.touches[0].clientY } : null;
  }

  function up(e) {
    if (!start) return;
    const touch = e.changedTouches[0];
    const dx = touch.clientX - start.x;
    const dy = touch.clientY - start.y;
    start = null;
    if (Math.abs(dx) >= MIN_PX && Math.abs(dx) > Math.abs(dy) * SIDEWAYS) handler?.(dx < 0 ? 1 : -1);
  }

  function cancel() {
    start = null;
  }

  node.addEventListener('touchstart', down, { passive: true });
  node.addEventListener('touchend', up, { passive: true });
  node.addEventListener('touchcancel', cancel, { passive: true });
  return {
    update(next) {
      handler = next;
    },
    destroy() {
      node.removeEventListener('touchstart', down);
      node.removeEventListener('touchend', up);
      node.removeEventListener('touchcancel', cancel);
    },
  };
}
