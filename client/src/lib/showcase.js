// The showcase: the public product demo at castle.dogs.red/demo/.
//
// It is the ordinary /demo, built with VITE_SHOWCASE=1, and it is strictly look-don't-touch:
// always the sample data, no Live mode and no box to connect to, and nothing in it runs or
// changes. Every page, card, detail, search and person is there to be looked at; every
// button that would start, save, move or delete something says so in a toast instead.
//
// How it is enforced: every demo source goes through readOnly() below. Its methods are let
// through only when their name is on READS, the list of methods that just read; anything
// else answers with REFUSED. Outside the showcase build, readOnly() hands the source back
// untouched, so the everyday /demo still pretends (a clone "starts", a rename sticks until
// a reload).
//
// Adding a section with a demo source of its own:
//   1. wrap it where shell.svelte.js makes it:   demoContacts = readOnly(new DemoContacts());
//      (or readOnly(new DemoContacts(), ['contactsList', 'contactPhoto']) for its reads)
//   2. put the names of its methods that only read on READS, or pass them as above.
//      A method missing from the list is refused, so a forgotten read shows up straight
//      away as a toast, never as data quietly changing.
//   3. call its writes inside try/catch and toast err.message, like every section does.
//      The refusal toasts once by itself; the same words toasted again are not repeated.
// Changes a component makes to its own demo state, without a source, check SHOWCASE and
// call refused() instead (see ai/DemoModels.svelte and SettingsSection.svelte).

export const SHOWCASE = import.meta.env.VITE_SHOWCASE === '1';

export const REFUSED = 'This is the demo. Look around: nothing here runs.';
export const WAITLIST_URL = 'https://castle.dogs.red/#top';

/** Methods that only read. Every demo source shares the list; names are unique enough. */
const READS = new Set([
  // Storage (the demo drive and its pretend computer)
  'listChildren', 'get', 'all', 'search', 'recent', 'stats', 'objectUrl', 'download', 'hostDrives',
  'attachPreviews', 'fileUrl', 'toNode', 'toHostNode', 'dispose',
  // Arcade and the Steam cache
  'arcadeSystems', 'arcadeGames', 'arcadeArt', 'arcadeAbout', 'arcadeSaves', 'arcadeSaveHistory', 'demoSaves',
  'steamCache', 'steamArt', 'steamSearch', 'steamLocal', 'steamRefresh',
  // Clones
  'clones', 'cloneMachines', 'cloneScreen', 'botView', 'view', 'find',
  // Theater (a demo "play" opens the sample picture; nothing is streamed)
  'theater', 'theaterItems', 'theaterArt', 'theaterDetails', 'theaterPlay', 'theaterFolders',
  // Network, the box, Settings and Security
  'network', 'networkProvider', 'system', 'settings', 'boxInfo',
  'security', 'securityLog', 'securityVerify', 'securityAudits', 'record',
  // Presence (radar nodes) and the spec card
  'presence', 'specs',
  // AI
  'aiStatus',
]);

let notify = () => {};

/** Where a refusal is shown. The store registers its toast here (avoids an import loop). */
export function onRefused(show) {
  notify = show;
}

/** Says no, once, in Castle's voice. For components that change their own demo state. */
export function refused() {
  notify(REFUSED);
}

/** The error a refused call rejects with, so callers that toast err.message say the same. */
export function refusal() {
  return Object.assign(new Error(REFUSED), { showcase: true });
}

/**
 * A demo source that can only be read, in the showcase build. Anywhere else, the source.
 * @template T
 * @param {T} source
 * @param {string[]} [reads] more method names of this source that only read
 * @returns {T}
 */
export function readOnly(source, reads = []) {
  if (!SHOWCASE || !source || typeof source !== 'object') return source;
  const allowed = reads.length ? new Set([...READS, ...reads]) : READS;
  return new Proxy(source, {
    get(target, prop) {
      // Read from the source itself, so getters and #private fields keep working.
      const value = Reflect.get(target, prop);
      if (typeof value !== 'function' || typeof prop !== 'string') return value;
      // Reads run on the source itself: what they call inside is not checked again.
      if (allowed.has(prop)) return value.bind(target);
      return () => {
        refused();
        return Promise.reject(refusal());
      };
    },
  });
}
