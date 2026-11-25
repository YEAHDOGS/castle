// Things shared to Castle from outside it, which today means one thing: a link to save.
//
// Two ways in:
//   * On Android, another app's Share menu lists Castle. The app shell keeps what was shared
//     and hands it over through `window.CastleShare.take()`, once (see
//     app/scripts/android-share.mjs, which adds that to the Android project).
//   * In a browser, an address ending in `?save=<link>`: a bookmark that sends the page
//     being read to the box.
// Either way the person lands in Links with the address being saved (shell.focus.add).

import { shell } from './shell.svelte.js';

const SAVE_PARAM = 'save';

/** What was shared since the last look, or ''. Taking it clears it. */
function takeShared() {
  try {
    const fromPhone = window.CastleShare?.take?.();
    if (fromPhone) return String(fromPhone);
  } catch {
    /* no bridge on this device */
  }
  const params = new URLSearchParams(location.search);
  const fromAddress = params.get(SAVE_PARAM);
  if (!fromAddress) return '';
  // Out of the address bar, so a reload does not save it twice.
  params.delete(SAVE_PARAM);
  const rest = params.toString();
  history.replaceState(null, '', location.pathname + (rest ? `?${rest}` : '') + location.hash);
  return fromAddress;
}

function look() {
  const shared = takeShared().trim();
  if (shared) shell.openWith('links', { add: shared });
}

/**
 * Looks for a share now, and again whenever the app comes back to the front (a share
 * while Castle was already open). Returns the function that stops looking.
 */
export function watchForShares() {
  if (typeof window === 'undefined') return () => {};
  const onShow = () => {
    if (document.visibilityState === 'visible') look();
  };
  look();
  document.addEventListener('visibilitychange', onShow);
  window.addEventListener('focus', look);
  return () => {
    document.removeEventListener('visibilitychange', onShow);
    window.removeEventListener('focus', look);
  };
}
