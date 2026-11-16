// The tour: what a first visit to the demo is told, and what it has already been told.
//
// Two layers (see Tour.svelte). First the carousel: one to three big cards, shown once, that
// say what Castle is, that this is sample data, and how the wheel works. Then the blurbs:
// comic-book speech bubbles anchored to real parts of the interface (BLURBS in blurbs.js),
// each put away on its own or all at once.
//
// What has been seen is kept in localStorage, so a second visit is left alone. The showcase
// (the public demo, lib/showcase.js) turns the tour on by itself the first time; the
// everyday app only shows it from Settings > Look and feel > Show the tour again.

import { SvelteSet } from 'svelte/reactivity';
import { SHOWCASE } from '../../lib/showcase.js';
import { BLURBS } from './blurbs.js';

const KEY = 'castle-tour-v1';

function readSaved() {
  try {
    return JSON.parse(localStorage.getItem(KEY)) || {};
  } catch {
    return {};
  }
}

class Tour {
  /** The tour is showing (either layer). Off, it costs nothing: Tour.svelte renders nothing. */
  on = $state(false);
  /** The carousel is the layer showing; false means the blurbs are. */
  carousel = $state(false);
  /** Ids of blurbs that were put away. */
  dismissed = new SvelteSet();

  constructor() {
    const saved = readSaved();
    for (const id of saved.dismissed || []) this.dismissed.add(id);
    // A first visit to the showcase gets the whole tour; one that has put it all away does not.
    if (SHOWCASE && !saved.done) {
      this.on = true;
      this.carousel = !saved.carouselSeen;
    }
  }

  /** The carousel is over (skipped or read to the end): the blurbs take over. */
  endCarousel() {
    this.carousel = false;
    this.save({ carouselSeen: true });
  }

  /** Puts one blurb away for good. Once every blurb is gone, the tour is over. */
  dismiss(id) {
    this.dismissed.add(id);
    const done = BLURBS.every((b) => this.dismissed.has(b.id));
    if (done) this.on = false;
    this.save({ dismissed: [...this.dismissed], done });
  }

  /** Puts every blurb away and ends the tour. */
  dismissAll() {
    for (const b of BLURBS) this.dismissed.add(b.id);
    this.on = false;
    this.save({ dismissed: [...this.dismissed], done: true });
  }

  /** Settings > Show the tour again: from the first card, as on a first visit. */
  restart() {
    this.dismissed.clear();
    this.save({ carouselSeen: false, dismissed: [], done: false });
    this.carousel = true;
    this.on = true;
  }

  save(changes) {
    try {
      localStorage.setItem(KEY, JSON.stringify({ ...readSaved(), ...changes }));
    } catch {
      /* storage blocked: the tour will show again next visit */
    }
  }
}

export const tour = new Tour();
