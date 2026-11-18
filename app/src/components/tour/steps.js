// The tutorials: what each section's pop-ups say and what they point at, in the order they
// are stepped through. A section has four at most (MAX_STEPS in tour.svelte.js cuts a longer
// list short); a section with no entry has no tutorial.
//
//   id      names the step (keys its words as they change)
//   title   two or three words, set large
//   text    one or two short sentences
//   demo    a sentence added to the text while sample data is showing (the demo, the showcase)
//   target  the data-tour attribute of the element it points at (set in the shell components).
//           A step with no target sits in the middle of the screen: it is about the whole thing.
//   or      a stand-in target when the first is not on screen (the rail is hidden on a phone;
//           Storage's bottom tabs and SectionFrame's chips stand in for it)
//   side    which side of the target the pop-up prefers (place.js moves it when there is no room)
//   at      where along the target's edge the tail lands, 0 to 1 (default: the middle)
//
// Castle's voice: plain words, no exclamation marks.

export const TOURS = {
  home: [
    { id: 'castle', title: 'This is Castle', text: 'A home server you own: files, photos, messages, games and films, on a box in your house.', demo: 'This is sample data and nothing is kept, so click anything.' },
    { id: 'wheel', target: 'wheel', side: 'bottom', at: 0.5, title: 'Turn the wheel', text: 'Every section lives on this wheel. Scroll over the corner button, or click it, and turn. The one under the beam opens.' },
    { id: 'tile-storage', target: 'tile-storage', side: 'bottom', at: 0.5, title: 'Start with Storage', text: 'Files, photos and backups, like a drive that lives in your house.' },
    { id: 'tile-comms', target: 'tile-comms', side: 'bottom', at: 0.5, title: 'Then Comms', text: 'It keeps texts, calls and email on the box, off the phone that will get lost.' },
  ],
  storage: [
    { id: 'topbar-storage', target: 'topbar', side: 'bottom', at: 0.3, title: 'Search and sort', text: 'Search the drive here. Grid or list, sort, and New are along the bar.' },
    { id: 'rail-storage', target: 'sidebar', or: 'tabs', side: 'right', at: 0.5, title: 'The views', text: 'Drive, Shared, Recent, Starred, Trash: the views of the drive.' },
  ],
  comms: [
    { id: 'topbar-comms', target: 'topbar', side: 'bottom', at: 0.25, title: 'Conversations', text: 'A conversation is one person on one channel. Open one and read it like a phone.' },
    { id: 'rail-comms', target: 'sidebar', or: 'tabs', side: 'right', at: 0.5, title: 'Channels', text: 'Texts, calls, email, Discord, Messenger: a row for every channel that has something.' },
  ],
};
