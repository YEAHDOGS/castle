// Shell state: which section is showing, who is using the app, and the two shell menus
// (the section wheel, top right; the people menu, top left).

import { store } from './store.svelte.js';
import { SECTIONS, DEFAULT_SECTION } from './sections.js';
import { DEMO_USERS, DEMO_PIN, OWNER, DemoArcade, DemoClones, DemoNetwork, DemoBox, DemoSecurity, DemoTheater } from './demo-shell.js';
import { prefs } from './prefs.svelte.js';

const SECTION_KEY = 'castle-section';
const WRONG_PIN = 'Wrong PIN';

function readSection() {
  // A link can name the section to open, e.g. /demo?section=security.
  const linked = typeof location === 'undefined' ? null : new URLSearchParams(location.search).get('section');
  if (SECTIONS.some((s) => s.id === linked)) return linked;
  // Settings > Look and feel > Open Castle in: a named section wins over "where I left off".
  const start = prefs.get('look.startSection');
  if (SECTIONS.some((s) => s.id === start)) return start;
  try {
    const saved = sessionStorage.getItem(SECTION_KEY);
    return SECTIONS.some((s) => s.id === saved) ? saved : DEFAULT_SECTION;
  } catch {
    return DEFAULT_SECTION;
  }
}

class Shell {
  section = $state(readSection());
  wheelOpen = $state(false);
  userMenuOpen = $state(false);
  userId = $state(null); // null = the first person in `users`

  // Demo stand-ins, kept for the whole visit so a pretend launch survives a section change.
  demoArcade = new DemoArcade();
  demoClones = new DemoClones();
  demoNetwork = new DemoNetwork();
  demoBox = new DemoBox();
  demoSecurity = new DemoSecurity();
  demoTheater = new DemoTheater();

  /** People on this box. Accounts exist only in the demo until the box grows them. */
  get users() {
    return store.isDemo ? DEMO_USERS : [OWNER];
  }

  get user() {
    return this.users.find((u) => u.id === this.userId) || this.users[0];
  }

  setSection(id) {
    if (!SECTIONS.some((s) => s.id === id)) return;
    this.section = id;
    try {
      sessionStorage.setItem(SECTION_KEY, id);
    } catch {
      /* storage blocked: the section just won't survive a reload */
    }
  }

  /** Back to Home from anywhere, closing whichever shell menu is open. */
  goHome() {
    this.wheelOpen = false;
    this.userMenuOpen = false;
    this.setSection(DEFAULT_SECTION);
  }

  /** Live or demo. People differ between the two, so the switch lands on the first one. */
  async setDemo(on) {
    this.userId = null;
    this.userMenuOpen = false;
    await store.setDemo(on);
    store.toast(on ? 'Demo: sample data, nothing is saved' : store.boxUrl ? 'Live: your box' : 'Live: no box connected');
  }

  /**
   * Switches person. A locked person needs their PIN; the demo's is DEMO_PIN.
   * @returns {string | null} an error to show, or null when the switch happened
   */
  async switchUser(id, pin = '') {
    const next = this.users.find((u) => u.id === id);
    if (!next) return null;
    if (next.locked && pin !== DEMO_PIN) return WRONG_PIN;
    this.userId = id;
    this.userMenuOpen = false;
    await store.showDriveOf(next);
    store.toast(`Signed in as ${next.name}`);
    return null;
  }

  /** Where Arcade reads from: the box, the demo, or nothing (no box connected). */
  get arcadeSource() {
    if (store.isDemo) return this.demoArcade;
    return store.boxUrl ? store.adapter : null; // boxUrl is reactive; the adapter object is not
  }

  /** Where Theater reads from, by the same rule. */
  get theaterSource() {
    if (store.isDemo) return this.demoTheater;
    return store.boxUrl ? store.adapter : null;
  }

  /** Where Network reads from, by the same rule. */
  get networkSource() {
    if (store.isDemo) return this.demoNetwork;
    return store.boxUrl ? store.adapter : null;
  }

  /** Where Security reads from, by the same rule. */
  get securitySource() {
    if (store.isDemo) return this.demoSecurity;
    return store.boxUrl ? store.adapter : null;
  }

  /** Where box settings and box facts come from, by the same rule. */
  get boxSource() {
    if (store.isDemo) return this.demoBox;
    return store.boxUrl ? store.adapter : null;
  }

  /** Where Clones reads from, by the same rule. */
  get clonesSource() {
    if (store.isDemo) return this.demoClones;
    return store.boxUrl ? store.adapter : null; // boxUrl is reactive; the adapter object is not
  }
}

export const shell = new Shell();
