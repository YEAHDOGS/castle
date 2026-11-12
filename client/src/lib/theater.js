// Theater's rules in one place: what each kind of library is called, who may see what,
// how items group into shows and albums, and how a search matches.

import { Film, Tv, Music, Image, Clapperboard } from '@lucide/svelte';

export const KINDS = [
  { id: 'movies', label: 'Movies', icon: Film, item: 'movie', shape: 'poster' },
  { id: 'shows', label: 'Shows', icon: Tv, item: 'episode', shape: 'poster' },
  { id: 'music', label: 'Music', icon: Music, item: 'track', shape: 'square' },
  { id: 'photos', label: 'Photos', icon: Image, item: 'photo', shape: 'wide' },
  { id: 'videos', label: 'Home videos', icon: Clapperboard, item: 'video', shape: 'wide' },
];
export const kindById = (id) => KINDS.find((k) => k.id === id) || KINDS[0];

/** Who a library is for, in the order they widen. */
export const AUDIENCES = [
  { id: 'everyone', label: 'Everyone', help: 'Every person on the box, guests and children too.' },
  { id: 'family', label: 'Family', help: 'Everyone but guests.' },
  { id: 'adults', label: 'Adults', help: 'Not children, not guests.' },
  { id: 'admins', label: 'Admins only', help: 'Only the people who run the box.' },
];

const SEES = {
  Admin: ['everyone', 'family', 'adults', 'admins'],
  Member: ['everyone', 'family', 'adults'],
  Child: ['everyone', 'family'],
  Guest: ['everyone'],
};

/** May this person see this library? */
export const canSeeLibrary = (role, library) => (SEES[role] || SEES.Guest).includes(library.audience);

/** May this person see this item? Adults-only items are hidden from children and guests. */
export function canSeeItem(role, item, libraries) {
  const library = libraries.find((l) => l.id === item.library);
  if (!library || !canSeeLibrary(role, library)) return false;
  return !(item.adultsOnly && (role === 'Child' || role === 'Guest'));
}

/** Does a search match an item? Title, show, artist, album, year, genre, folder and tags. */
export function matches(item, query) {
  const q = query.trim().toLowerCase();
  if (!q) return true;
  const hay = [item.title, item.show, item.artist, item.album, item.folder, item.genre, item.year || '', ...(item.tags || [])].join(' ').toLowerCase();
  return q.split(/\s+/).every((word) => hay.includes(word));
}

/**
 * The cards a kind shows. Films, photos and videos are one card each; episodes group into
 * shows and songs into albums. Each card: { key, title, subtitle, items, lead, year, added }.
 */
export function cardsFor(kind, items) {
  if (kind === 'shows' || kind === 'music') {
    const groups = new Map();
    for (const item of items) {
      const key = kind === 'shows' ? `show:${item.show.toLowerCase()}` : `album:${item.artist.toLowerCase()}|${item.album.toLowerCase()}`;
      if (!groups.has(key)) groups.set(key, []);
      groups.get(key).push(item);
    }
    return [...groups.entries()].map(([key, group]) => {
      group.sort(kind === 'shows' ? (a, b) => a.season - b.season || a.episode - b.episode : (a, b) => a.track - b.track || a.title.localeCompare(b.title));
      const lead = group[0];
      const seasons = new Set(group.map((i) => i.season)).size;
      return {
        key,
        title: kind === 'shows' ? lead.show || 'Unknown show' : lead.album || 'Unknown album',
        subtitle: kind === 'shows' ? `${seasons > 1 ? `${seasons} seasons · ` : ''}${group.length} episode${group.length === 1 ? '' : 's'}` : lead.artist || `${group.length} songs`,
        items: group,
        lead,
        year: Math.min(...group.map((i) => i.year || 9999)) % 9999,
        added: Math.max(...group.map((i) => i.added)),
      };
    });
  }
  return items.map((item) => ({
    key: item.id,
    title: item.title,
    subtitle: kind === 'photos' || kind === 'videos' ? item.folder : item.year ? String(item.year) : '',
    items: [item],
    lead: item,
    year: item.year,
    added: item.added,
  }));
}

export const SORTS = [
  { id: 'title', label: 'A–Z' },
  { id: 'added', label: 'NEWEST' },
  { id: 'year', label: 'YEAR' },
];

export function sortCards(cards, by) {
  const list = [...cards];
  if (by === 'added') list.sort((a, b) => b.added - a.added);
  else if (by === 'year') list.sort((a, b) => (b.year || 0) - (a.year || 0) || a.title.localeCompare(b.title));
  else list.sort((a, b) => a.title.localeCompare(b.title, undefined, { sensitivity: 'base', numeric: true }));
  return list;
}

/** 7384 -> "2 h 3 min"; 245 -> "4:05". */
export function fmtLength(seconds, long = true) {
  if (!seconds) return '';
  const s = Math.round(seconds);
  if (!long || s < 3600) {
    const m = Math.floor(s / 60);
    return long && s >= 600 ? `${m} min` : `${m}:${String(s % 60).padStart(2, '0')}`;
  }
  return `${Math.floor(s / 3600)} h ${Math.round((s % 3600) / 60)} min`;
}

/** "S01E03" */
export const episodeCode = (item) => `S${String(item.season).padStart(2, '0')}E${String(item.episode).padStart(2, '0')}`;

/** 1920x1080 -> "1080p"; 3840x2160 -> "4K". */
export function resolution(probe) {
  if (!probe?.height) return '';
  if (probe.height >= 2000) return '4K';
  return `${probe.height >= 1000 ? 1080 : probe.height >= 700 ? 720 : probe.height}p`;
}
