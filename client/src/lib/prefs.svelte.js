// Device settings: the ones that belong to this browser or app, not to the box. They are
// kept in localStorage and read wherever they apply (the wheel, Arcade, Clones, motion).

import { defaultsFor } from './settings.js';

const PREFS_KEY = 'castle-prefs-v1';
const REDUCED = 'reduced';

function readSaved() {
  try {
    return JSON.parse(localStorage.getItem(PREFS_KEY)) || {};
  } catch {
    return {};
  }
}

class Prefs {
  values = $state({ ...defaultsFor('device'), ...readSaved() });

  constructor() {
    this.applyMotion();
  }

  get(id) {
    return this.values[id];
  }

  set(id, value) {
    this.values[id] = value;
    try {
      localStorage.setItem(PREFS_KEY, JSON.stringify(this.values));
    } catch {
      /* storage blocked: the change lasts for this visit only */
    }
    if (id === 'look.motion') this.applyMotion();
  }

  /** Marks the page so CSS and the transition helpers both know motion is reduced. */
  applyMotion() {
    if (typeof document === 'undefined') return;
    document.documentElement.dataset.motion = this.values['look.motion'] === REDUCED ? REDUCED : 'full';
  }
}

export const prefs = new Prefs();
