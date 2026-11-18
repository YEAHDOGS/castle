// The tour: a short tutorial on each section, as a stepper. One pop-up at a time, Back and
// Next at the left and right edges of the window, four pop-ups at most (Steps.svelte draws
// it; the words are steps.js).
//
// Which sections have been toured is kept in localStorage, so each section's tutorial shows
// once. The showcase (the public demo, lib/showcase.js) starts a section's tutorial by itself
// on a first visit to that section; the everyday app only does after Settings > Look and
// feel > Show the tutorial again was pressed.

import { SHOWCASE } from '../../lib/showcase.js';
import { TOURS } from './steps.js';

const KEY = 'castle-tour-v2';
export const MAX_STEPS = 4;

/** @returns {{ seen?: Record<string, boolean>, asked?: boolean }} what this device remembers of the tour */
function readSaved() {
  try {
    return JSON.parse(localStorage.getItem(KEY)) || {};
  } catch {
    return {};
  }
}

class Tour {
  /** A tutorial is showing. Off, it costs nothing: Tour.svelte renders nothing. */
  on = $state(false);
  /** The step showing, from zero. */
  at = $state(0);
  /** Which way the last step went: 1 on, -1 back. The words come in from that side. */
  dir = $state(1);
  /** The section showing, whose tutorial this is. */
  section = $state('');

  /** A section's steps, never more than MAX_STEPS. */
  steps(section = this.section) {
    return (TOURS[section] || []).slice(0, MAX_STEPS);
  }

  /**
   * Tour.svelte calls this with every section shown. Leaving a section in the middle of its
   * tutorial counts as skipping it; a first visit to the new one starts its own.
   */
  arrive(section) {
    if (section === this.section) return;
    if (this.on) this.finish();
    this.section = section;
    const saved = readSaved();
    if ((SHOWCASE || saved.asked) && !saved.seen?.[section]) this.start();
  }

  /** This section's tutorial, from its first step. A section without one shows nothing. */
  start() {
    if (!this.steps().length) return;
    this.at = 0;
    this.dir = 1;
    this.on = true;
  }

  /** Settings > Show the tutorial again: every section is new again, and this one's replays now. */
  restart() {
    this.save({ asked: true, seen: {} });
    this.start();
  }

  /** One step on. Past the last one, the tutorial is over. */
  next() {
    if (this.at >= this.steps().length - 1) {
      this.finish();
      return;
    }
    this.dir = 1;
    this.at += 1;
  }

  back() {
    if (this.at === 0) return;
    this.dir = -1;
    this.at -= 1;
  }

  /** Ends the tutorial (read to the end, or skipped) and remembers this section had it. */
  finish() {
    this.on = false;
    this.save({ seen: { ...readSaved().seen, [this.section]: true } });
  }

  save(changes) {
    try {
      localStorage.setItem(KEY, JSON.stringify({ ...readSaved(), ...changes }));
    } catch {
      /* storage blocked: the tutorial will show again next visit */
    }
  }
}

export const tour = new Tour();
