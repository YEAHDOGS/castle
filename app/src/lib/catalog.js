// A music catalog kept in a bucket: one small JSON file a website reads to know which
// songs it has, so a new song goes live by uploading it and saving the catalog, with no
// change to the site's own code.
//
// The bucket decides whether there is one. Castle shows the Catalog panel only when the
// connected bucket holds this file and it says it is this kind:
//
//   music/catalog.json
//   { "version": 1, "kind": "dogs-music",
//     "audioFolder": "music/{year}/", "coverFolder": "img/covers/{year}/",
//     "tracks": [ { "id", "title", "artist", "album", "cover", "altCover", "src",
//                   "instrumental", "dateAdded", "year", "genre", "public" } ] }
//
// `src` is the full mix and `instrumental` the one without vocals; a track has at least one.
// The folders say where a new song's files are uploaded, with {year} filled in.

import { BUCKET_PREFIX, slug } from './bucket.js';

export const CATALOG_KEY = 'music/catalog.json';
export const CATALOG_KIND = 'dogs-music';
export const CATALOG_VERSION = 1;
const DEFAULT_AUDIO_FOLDER = 'music/{year}/';
const DEFAULT_COVER_FOLDER = 'img/covers/{year}/';
const YEAR_SLOT = '{year}';
const MIN_YEAR = 1900;
const FALLBACK_ID = 'track';
/** The text fields of a track, in the order the site lists them. */
const TEXT_FIELDS = ['id', 'title', 'artist', 'album', 'cover', 'altCover', 'src', 'instrumental', 'dateAdded', 'genre', 'attrib'];

/**
 * Reads a catalog file. Null when the text is not a catalog of this kind, so a bucket
 * with some other "music/catalog.json" is left alone.
 * @param {string | null} text
 * @returns {{ version: number, kind: string, audioFolder: string, coverFolder: string, tracks: object[] } | null}
 */
export function parseCatalog(text) {
  if (!text) return null;
  let raw;
  try {
    raw = JSON.parse(text);
  } catch {
    return null;
  }
  if (!raw || raw.kind !== CATALOG_KIND || !Array.isArray(raw.tracks)) return null;
  return {
    version: CATALOG_VERSION,
    kind: CATALOG_KIND,
    audioFolder: folderOr(raw.audioFolder, DEFAULT_AUDIO_FOLDER),
    coverFolder: folderOr(raw.coverFolder, DEFAULT_COVER_FOLDER),
    tracks: raw.tracks.filter((t) => t && typeof t.id === 'string' && typeof t.title === 'string'),
  };
}

const folderOr = (value, fallback) => (typeof value === 'string' && /^[^/].*\/$/.test(value) ? value : fallback);

/** The catalog as the file's text, with when it was saved. */
export function catalogText(catalog) {
  return JSON.stringify({ ...catalog, updated: new Date().toISOString() }, null, 2) + '\n';
}

/** "music/{year}/" with the year filled in. */
export const folderFor = (template, year) => template.split(YEAR_SLOT).join(String(year));

/** An id from the title that no other track has: "gta6-freestyle", then "gta6-freestyle-2". */
export function freeId(title, tracks) {
  const base = slug(title) || FALLBACK_ID;
  const taken = new Set(tracks.map((t) => t.id));
  if (!taken.has(base)) return base;
  for (let n = 2; ; n++) if (!taken.has(`${base}-${n}`)) return `${base}-${n}`;
}

/** A year typed into the form, or this year when it is not one. */
export function cleanYear(raw, now = new Date()) {
  const year = Math.round(Number(raw));
  return year >= MIN_YEAR && year <= now.getFullYear() + 1 ? year : now.getFullYear();
}

/**
 * A track as the catalog keeps it: trimmed text, a real year, and nothing but its own fields.
 * @param {object} fields what the form holds
 */
export function cleanTrack(fields) {
  const track = {};
  for (const key of TEXT_FIELDS) {
    const value = typeof fields[key] === 'string' ? fields[key].trim() : '';
    if (value || key !== 'attrib') track[key] = value;
  }
  track.year = cleanYear(fields.year);
  track.public = fields.public !== false;
  return track;
}

/** What is wrong with a track, in words, or '' when it can be saved. */
export function trackProblem(track) {
  if (!track.title) return 'The song needs a title.';
  if (!track.artist) return 'The song needs an artist.';
  if (!track.src && !track.instrumental) return 'Pick the song file, or its instrumental.';
  return '';
}

/** The catalog with `track` added, or put in place of the one with its id. */
export function withTrack(catalog, track) {
  const at = catalog.tracks.findIndex((t) => t.id === track.id);
  const tracks = at === -1 ? [...catalog.tracks, track] : catalog.tracks.map((t, i) => (i === at ? { ...t, ...track } : t));
  return { ...catalog, tracks };
}

export const withoutTrack = (catalog, id) => ({ ...catalog, tracks: catalog.tracks.filter((t) => t.id !== id) });

// ---- Saving: the catalog in the bucket is read again first, so two devices do not undo each other ----

const GONE = 'The catalog is no longer in the bucket.';
const NO_PUBLIC_ADDRESS = 'This bucket has no public address, so a song in it has no link. Disconnect, then connect it again with one.';
/** Each file of a song: its field in the track, what to call it, and which folder it goes in. */
const UPLOADS = [
  { field: 'src', label: 'the song', folder: 'audioFolder' },
  { field: 'instrumental', label: 'the instrumental', folder: 'audioFolder' },
  { field: 'cover', label: 'the cover', folder: 'coverFolder' },
  { field: 'altCover', label: 'the second cover', folder: 'coverFolder' },
];

async function latestCatalog(adapter) {
  const catalog = parseCatalog(await adapter.bucketRead(CATALOG_KEY));
  if (!catalog) throw new Error(GONE);
  return catalog;
}

/**
 * Uploads a song's files and puts it in the catalog: a new song, or changes to one already
 * there (`fields.id` set). A file left out keeps the address the track already has.
 * @param {object} adapter the box (HttpAdapter)
 * @param {object} fields the form: title, artist, album, genre, year, public, and for an edit the track's own fields
 * @param {{ src?: File, instrumental?: File, cover?: File, altCover?: File }} files
 * @param {(step: string) => void} say told what is happening, in words
 * @returns {Promise<object>} the catalog as saved
 */
export async function publishTrack(adapter, fields, files, say = () => {}) {
  const planned = cleanTrack({ ...fields, ...Object.fromEntries(UPLOADS.filter((u) => files[u.field]).map((u) => [u.field, 'pending'])) });
  const problem = trackProblem(planned);
  if (problem) throw new Error(problem);
  const catalog = await latestCatalog(adapter);
  const year = cleanYear(fields.year);
  const uploaded = {};
  for (const { field, label, folder } of UPLOADS) {
    if (!files[field]) continue;
    say(`Uploading ${label}`);
    const [node] = await adapter.upload([files[field]], BUCKET_PREFIX + folderFor(catalog[folder], year));
    if (!node.url) throw new Error(NO_PUBLIC_ADDRESS);
    uploaded[field] = node.url;
  }
  say('Saving the catalog');
  const track = cleanTrack({
    ...fields,
    ...uploaded,
    id: fields.id || freeId(fields.title, catalog.tracks),
    dateAdded: fields.dateAdded || new Date().toISOString(),
  });
  const next = withTrack(catalog, track);
  await adapter.bucketWrite(CATALOG_KEY, catalogText(next));
  return next;
}

/** Takes a song out of the catalog. Its files stay in the bucket. */
export async function unpublishTrack(adapter, id) {
  const next = withoutTrack(await latestCatalog(adapter), id);
  await adapter.bucketWrite(CATALOG_KEY, catalogText(next));
  return next;
}
