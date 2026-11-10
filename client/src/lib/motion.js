// Motion language — the wearedogs.net expo-out dialect, tuned for Castle.
// LAW: transform/opacity only. Fast > fancy. Motion never blocks input.
// Speeds: 150ms hovers, 250ms UI, 320ms moments. Easing: expo-out everywhere.

export const EXPO_BEZIER = 'cubic-bezier(0.16, 1, 0.3, 1)';

/** JS mirror of cubic-bezier(0.16,1,0.3,1) for Svelte transition params. */
export const expoOut = (t) => (t === 1 ? 1 : 1 - Math.pow(2, -10 * t));

export const DUR = { HOVER: 150, UI: 250, MOMENT: 320 };

/** True when the OS asks for stillness — skip staggers and flourishes. */
export function reducedMotion() {
  // Either the operating system asks for it, or the person chose Reduced in Settings.
  if (typeof document !== 'undefined' && document.documentElement.dataset.motion === 'reduced') return true;
  return (
    typeof matchMedia !== 'undefined' &&
    matchMedia('(prefers-reduced-motion: reduce)').matches
  );
}

/** Stagger delay for list/grid entry, capped so big folders don't crawl. */
export function stagger(i, step = 18, cap = 180) {
  return reducedMotion() ? 0 : Math.min(i * step, cap);
}

/** View-change transition: folder drill-in / view switch. Directional slide + fade. */
export function viewSwap(dir = 1) {
  if (reducedMotion()) return { duration: 0 };
  return { duration: DUR.UI, easing: expoOut, x: 28 * dir, opacity: 0 };
}

/** Pop transition: context menus, dropdowns, toasts. */
export function pop() {
  if (reducedMotion()) return { duration: 0 };
  return { duration: DUR.HOVER, easing: expoOut, scale: 0.96, opacity: 0 };
}

/** Modal transition: preview open/close. */
export function modalIn() {
  if (reducedMotion()) return { duration: 0 };
  return { duration: DUR.MOMENT, easing: expoOut, scale: 0.965, y: 10, opacity: 0 };
}

/** Grid/list item entry: small rise, staggered. */
export function itemIn(i = 0) {
  if (reducedMotion()) return { duration: 0 };
  return { duration: DUR.UI, delay: stagger(i), easing: expoOut, y: 10, opacity: 0 };
}

/** A section's view changing (a tab in its rail): the new view rises into place. */
export function viewIn() {
  if (reducedMotion()) return { duration: 0 };
  return { duration: DUR.UI, easing: expoOut, y: 12, opacity: 0 };
}

/** The whole interface changing section from the wheel. */
export function sectionIn() {
  if (reducedMotion()) return { duration: 0 };
  return { duration: DUR.MOMENT, easing: expoOut, start: 0.985, opacity: 0 };
}
