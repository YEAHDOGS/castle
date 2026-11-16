// The blurbs: what each speech bubble says and what it points at.
//
//   target   the data-tour attribute of the element it points at (set in the shell components)
//   or       a stand-in target when the first is not on screen (the rail is hidden on a phone;
//            Storage's bottom tabs and SectionFrame's chips stand in for it)
//   section  the section it belongs to; null shows it everywhere
//   side     which side of the target the bubble prefers (place.js moves it when there is no room)
//   at       where along the target's edge the tail lands, 0 to 1 (default: the middle)
//   kind     'speech' (a tail) or 'thought' (little circles)
//
// One or two short sentences each. Castle's voice: plain words, no exclamation marks.

export const BLURBS = [
  // ---- everywhere ----
  { id: 'wheel', target: 'wheel', section: null, side: 'left', at: 0.5, kind: 'speech', text: 'Every section lives on this wheel. Scroll over the corner, or click it, and turn.' },
  { id: 'strip', target: 'strip', section: null, side: 'top', at: 0.35, kind: 'thought', text: 'All of this is sample data. Nothing you do here is kept, so poke at anything.' },

  // ---- Home ----
  { id: 'rail-home', target: 'sidebar', section: 'home', side: 'right', at: 0.3, kind: 'speech', text: 'The rail. On Home it lists who uses this box; in a section, that section’s views.' },
  { id: 'tile-storage', target: 'tile-storage', section: 'home', side: 'bottom', at: 0.5, kind: 'speech', text: 'Start with Storage: files, photos and backups, like a drive that lives in your house.' },
  { id: 'tile-comms', target: 'tile-comms', section: 'home', side: 'bottom', at: 0.5, kind: 'speech', text: 'Comms keeps texts, calls and email on the box, off the phone that will get lost.' },

  // ---- Storage ----
  { id: 'topbar-storage', target: 'topbar', section: 'storage', side: 'bottom', at: 0.3, kind: 'speech', text: 'Search the drive here. Grid or list, sort, and New are along the bar.' },
  { id: 'rail-storage', target: 'sidebar', or: 'tabs', section: 'storage', side: 'right', at: 0.5, kind: 'speech', text: 'Drive, Shared, Recent, Starred, Trash: the views of the drive.' },

  // ---- Comms ----
  { id: 'topbar-comms', target: 'topbar', section: 'comms', side: 'bottom', at: 0.25, kind: 'speech', text: 'A conversation is one person on one channel. Open one and read it like a phone.' },
  { id: 'rail-comms', target: 'sidebar', or: 'tabs', section: 'comms', side: 'right', at: 0.5, kind: 'speech', text: 'Texts, calls, email, Discord, Messenger: a row for every channel that has something.' },
];
