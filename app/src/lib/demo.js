// The demo drive: a fixed set of files served at /demo.
//
// This is the proving ground. Every new feature is tried here first, against data that is
// the same on every visit, before it is pointed at a real box. Nothing here is saved: a
// reload puts everything back. When a feature needs a case that is not here (a new file
// type, a new state), add it to this file so the case stays covered.

import { svgThumb } from './adapter.js';

const DEMO_PATH = '/demo';
const ROOT = 'root';
const HOUR = 36e5;
const DAY = 864e5;
const KB = 1024;
const MB = 1024 ** 2;
const GB = 1024 ** 3;
const MIN_MS = 6e4;
const FIRST_PERSON_ID = 'alex'; // gets the full set below; everyone else gets personalNodes
const PHOTO_ROLL_COUNT = 36; // enough to scroll the grid and test range selection
const PHOTO_TINTS = [
  ['#8a5a2b', '#2b1a0e'], ['#123a4a', '#0e0b1e'], ['#3a1a4a', '#0b0e1e'],
  ['#1a4a2b', '#0e1e12'], ['#4a1a1a', '#1e0e0e'], ['#2b3a8a', '#0e122b'],
];
const FIELD_NOTES = 'FIELD NOTES\n\n- Uplink stable.\n- Vault at 41 percent.\n- Pack light. Move fast.\n';
const README_MD = '# Demo drive\n\nEverything here is sample data. Change anything you like: a reload puts it back.\n\n- [x] Folders, three levels deep\n- [x] Starred, shared and trashed items\n- [ ] Your next feature\n';
const SETTINGS_JSON = JSON.stringify({ theme: 'void', density: 'grid', sync: { enabled: true, intervalMinutes: 15 } }, null, 2);
const LONG_TEXT = Array.from({ length: 120 }, (_, i) => `Line ${i + 1}: the quick brown dog jumps over the lazy fence.`).join('\n');

/** True when the app was opened at /demo (with or without a trailing slash). */
export function isDemoRoute() {
  if (typeof location === 'undefined') return false;
  return location.pathname.replace(/\/+$/, '') === DEMO_PATH;
}

/**
 * The demo's files. Times are relative to now so "Today", "Yesterday" and older dates all
 * appear in Recent on any day the demo is opened.
 * @returns {object[]} nodes in the shape StorageAdapter documents
 */
export function demoNodes(person = null) {
  if (person && person.id !== FIRST_PERSON_ID) return personalNodes(person);
  const now = Date.now();
  const node = (id, name, parentId, extra = {}) => ({
    id, name, parentId, kind: 'file', mime: 'application/octet-stream', size: 0,
    modified: now - 3 * DAY, starred: false, trashed: false, shared: false, ...extra,
  });
  const folder = (id, name, parentId, extra = {}) => node(id, name, parentId, { kind: 'folder', mime: '', ...extra });
  const image = (id, name, parentId, tint, extra = {}) =>
    node(id, name, parentId, { mime: 'image/jpeg', size: 3.8 * MB, thumb: svgThumb(name.toUpperCase(), tint[0], tint[1]), ...extra });

  const photoRoll = Array.from({ length: PHOTO_ROLL_COUNT }, (_, i) => {
    const n = String(i + 1).padStart(3, '0');
    return image(`roll-${n}`, `IMG_${n}.jpg`, 'camera-roll', PHOTO_TINTS[i % PHOTO_TINTS.length], {
      size: (2 + (i % 5)) * MB,
      modified: now - (i + 2) * DAY,
    });
  });

  return [
    // ---- top level: one of each kind of thing ----
    folder('projects', 'Projects', ROOT, { modified: now - 2 * DAY, starred: true }),
    folder('photos', 'Photos', ROOT, { modified: now - 5 * DAY }),
    folder('music', 'Music', ROOT, { modified: now - 9 * DAY }),
    folder('backups', 'Backups', ROOT, { modified: now - 14 * DAY }),
    folder('family', 'Family Shared', ROOT, { modified: now - 1 * DAY, shared: true }),
    folder('empty', 'Empty folder', ROOT, { modified: now - 30 * DAY }),
    node('readme', 'README.md', ROOT, { mime: 'text/markdown', size: README_MD.length, modified: now - 1 * HOUR, text: README_MD, starred: true }),

    // ---- Projects: previews, nesting three deep ----
    folder('castle', 'Castle', 'projects', { modified: now - 3 * HOUR }),
    folder('castle-design', 'Design', 'castle', { modified: now - 4 * HOUR }),
    node('schematic', 'castle-schematic.svg', 'castle-design', { mime: 'image/svg+xml', size: 180 * KB, modified: now - 2 * HOUR, thumb: svgThumb('CASTLE // SCHEMATIC v3', '#1a2b4a', '#050508') }),
    node('moodboard', 'moodboard.png', 'castle-design', { mime: 'image/png', size: 6.2 * MB, modified: now - 26 * HOUR, thumb: svgThumb('MOODBOARD', '#4a2b1a', '#0b0508') }),
    node('field-notes', 'field-notes.txt', 'castle', { mime: 'text/plain', size: FIELD_NOTES.length, modified: now - 5 * HOUR, starred: true, text: FIELD_NOTES }),
    node('settings', 'settings.json', 'castle', { mime: 'application/json', size: SETTINGS_JSON.length, modified: now - 7 * HOUR, text: SETTINGS_JSON }),
    node('long-log', 'build-log.txt', 'castle', { mime: 'text/plain', size: LONG_TEXT.length, modified: now - 2 * DAY, text: LONG_TEXT }),
    node('pitch', 'pitch-v3.pdf', 'projects', { mime: 'application/pdf', size: 2.4 * MB, modified: now - 26 * HOUR }),
    node('budget', 'budget-2026.xlsx', 'projects', { mime: 'application/vnd.openxmlformats-officedocument.spreadsheetml.sheet', size: 88 * KB, modified: now - 6 * DAY }),
    node('long-name', 'a-very-long-file-name-that-keeps-going-to-test-how-the-grid-and-list-truncate-it-final-v2-really-final.docx', 'projects', { mime: 'application/vnd.openxmlformats-officedocument.wordprocessingml.document', size: 41 * KB, modified: now - 11 * DAY }),
    node('unicode-name', 'résumé — 履歴書 🐕.pdf', 'projects', { mime: 'application/pdf', size: 96 * KB, modified: now - 40 * DAY }),
    node('zero-bytes', 'empty-file.txt', 'projects', { mime: 'text/plain', size: 0, modified: now - 400 * DAY }),

    // ---- Photos: thumbnails, and a folder big enough to scroll ----
    image('dune-trip', 'dune-trip.jpg', 'photos', PHOTO_TINTS[0], { modified: now - 5 * DAY, starred: true }),
    image('neon-city', 'neon-city.jpg', 'photos', PHOTO_TINTS[1], { modified: now - 6 * DAY }),
    node('clip', 'beach-clip.mp4', 'photos', { mime: 'video/mp4', size: 148 * MB, modified: now - 8 * DAY }),
    folder('camera-roll', 'Camera Roll', 'photos', { modified: now - 2 * DAY }),
    ...photoRoll,

    // ---- Music ----
    node('loop', 'loop-04.wav', 'music', { mime: 'audio/wav', size: 44 * MB, modified: now - 9 * DAY }),
    node('demo-take', 'demo-take-2.mp3', 'music', { mime: 'audio/mpeg', size: 9.8 * MB, modified: now - 10 * DAY, starred: true }),

    // ---- Backups: very large files ----
    node('phone-backup', 'phone-2026-08.zip', 'backups', { mime: 'application/zip', size: 8.1 * GB, modified: now - 14 * DAY }),
    node('disk-image', 'laptop-disk.img', 'backups', { size: 512 * GB, modified: now - 90 * DAY }),

    // ---- Shared: a shared folder, its contents, and a lone shared file elsewhere ----
    node('manifest', 'trip-plan.md', 'family', { mime: 'text/markdown', size: 1.9 * KB, modified: now - 20 * HOUR, shared: true, text: '# Trip plan\n\n- [x] Flights\n- [x] Dog sitter\n- [ ] Pack\n' }),
    image('family-photo', 'reunion.jpg', 'family', PHOTO_TINTS[3], { modified: now - 3 * DAY, shared: true }),
    node('shared-video', 'birthday.mov', 'family', { mime: 'video/quicktime', size: 640 * MB, modified: now - 2 * DAY, shared: true }),
    node('shared-single', 'invoice-shared.pdf', 'projects', { mime: 'application/pdf', size: 210 * KB, modified: now - 4 * DAY, shared: true, starred: true }),

    // ---- Trash: a lone file, and a folder that took its contents with it ----
    node('old-resume', 'old-resume.pdf', ROOT, { mime: 'application/pdf', size: 96 * KB, modified: now - 60 * DAY, trashed: true }),
    image('blurry', 'blurry-shot.jpg', 'photos', PHOTO_TINTS[4], { modified: now - 12 * DAY, trashed: true }),
    folder('old-project', 'Old project', 'projects', { modified: now - 200 * DAY, trashed: true }),
    node('old-notes', 'notes-2024.txt', 'old-project', { mime: 'text/plain', size: 2 * KB, modified: now - 200 * DAY, trashed: true, text: 'Nothing here worth keeping.\n' }),
  ];
}

/**
 * A small drive for anyone but the first person, so switching people visibly changes
 * Storage: each person has their own files, and a guest sees shared items only.
 */
function personalNodes(person) {
  const now = Date.now();
  const tag = person.id;
  const file = (id, name, parentId, extra = {}) => ({
    id: `${tag}-${id}`, name, parentId, kind: 'file', mime: 'application/octet-stream', size: 0,
    modified: now - 2 * DAY, starred: false, trashed: false, shared: false, ...extra,
  });
  const dir = (id, name, extra = {}) => file(id, name, ROOT, { kind: 'folder', mime: '', ...extra });
  const shared = [
    dir('family', 'Family Shared', { shared: true, modified: now - 1 * DAY }),
    file('reunion', 'reunion.jpg', `${tag}-family`, { mime: 'image/jpeg', size: 3.8 * MB, shared: true, thumb: svgThumb('REUNION.JPG', PHOTO_TINTS[3][0], PHOTO_TINTS[3][1]) }),
    file('trip', 'trip-plan.md', `${tag}-family`, { mime: 'text/markdown', size: 1.9 * KB, shared: true, text: '# Trip plan\n\n- [x] Flights\n- [ ] Pack\n' }),
  ];
  if (person.role === 'Guest') return shared;
  return [
    ...shared,
    dir('docs', `${person.name}'s documents`, { modified: now - 3 * HOUR }),
    file('notes', 'notes.txt', `${tag}-docs`, { mime: 'text/plain', size: 64, modified: now - 3 * HOUR, starred: true, text: `Private to ${person.name}. Nobody else on the box can open this.\n` }),
    file('homework', person.role === 'Child' ? 'homework-week-12.pdf' : 'tax-2026.pdf', `${tag}-docs`, { mime: 'application/pdf', size: 420 * KB, modified: now - 4 * DAY }),
    dir('pics', 'Photos', { modified: now - 6 * DAY }),
    file('pic1', 'park.jpg', `${tag}-pics`, { mime: 'image/jpeg', size: 2.9 * MB, modified: now - 6 * DAY, thumb: svgThumb('PARK.JPG', PHOTO_TINTS[5][0], PHOTO_TINTS[5][1]) }),
    file('old', 'old-draft.txt', ROOT, { mime: 'text/plain', size: 12, trashed: true, text: 'Never mind.\n' }),
  ];
}

// ---- This computer: the machine the demo box pretends to run on ----

const DEMO_VAULT = 'C:\\Castle';
const DEMO_SERIALS = { c: '38FC6224', e: '5A1E09C3', f: 'C0FFEE42', d: '7D1B0A11', g: '0BADF00D', h: '5EC0DE12' };
const TB = 1e12;

/**
 * A computer with a system drive, a second-hand 12 TB disk that has been powered on for
 * 67,000 hours and has started growing bad sectors, a USB stick, an SD card and a disc in
 * the drive, plus a stick that was plugged in last week. Its folders are nodes with "host:" ids, so the same
 * browser opens them.
 */
export function demoComputer() {
  const now = Date.now();
  const at = (path) => `host:${path}`;
  const parentOf = (path) => {
    const cut = path.replace(/\\$/, '').lastIndexOf('\\');
    const up = path.slice(0, cut);
    return at(/^[A-Z]:$/.test(up) ? `${up}\\` : up);
  };
  const item = (path, extra = {}) => ({
    id: at(path), name: path.split('\\').pop(), parentId: parentOf(path), kind: 'file',
    mime: 'application/octet-stream', size: 0, modified: now - 4 * DAY,
    starred: false, trashed: false, shared: false, hidden: false, link: false, readOnly: false, ...extra,
  });
  const dir = (path, extra = {}) => item(path, { kind: 'folder', mime: '', ...extra });
  const pic = (path, tint, extra = {}) =>
    item(path, { mime: 'image/jpeg', size: 4.1 * MB, thumb: svgThumb(path.split('\\').pop().toUpperCase(), tint[0], tint[1]), ...extra });
  const shots = (folder, prefix, count, tint0) =>
    Array.from({ length: count }, (_, i) => pic(`${folder}\\${prefix}${String(i + 1).padStart(4, '0')}.JPG`, PHOTO_TINTS[(i + tint0) % PHOTO_TINTS.length], { modified: now - (i + 1) * HOUR }));

  const nodes = [
    // C: the system drive, with the vault on it
    dir('C:\\Castle', { modified: now - 1 * HOUR }),
    dir('C:\\Program Files', { modified: now - 20 * DAY }),
    dir('C:\\Users', { modified: now - 9 * DAY }),
    dir('C:\\Windows', { modified: now - 2 * DAY }),
    dir('C:\\$Recycle.Bin', { hidden: true }),
    item('C:\\pagefile.sys', { size: 12 * GB, hidden: true, modified: now - 1 * HOUR }),
    dir('C:\\Program Files\\Castle', { modified: now - 3 * DAY }),
    item('C:\\Program Files\\Castle\\castled.exe', { mime: 'application/x-msdownload', size: 9.6 * MB }),
    dir('C:\\Users\\alex', { modified: now - 1 * DAY }),
    dir('C:\\Users\\alex\\Desktop', { modified: now - 3 * HOUR }),
    dir('C:\\Users\\alex\\Documents', { modified: now - 2 * DAY }),
    dir('C:\\Users\\alex\\Downloads', { modified: now - 5 * HOUR }),
    dir('C:\\Users\\alex\\Pictures', { modified: now - 6 * DAY }),
    dir('C:\\Users\\alex\\.ssh', { hidden: true }),
    item('C:\\Users\\alex\\NTUSER.DAT', { size: 3 * MB, hidden: true }),
    item('C:\\Users\\alex\\Desktop\\todo.txt', { mime: 'text/plain', size: 61, modified: now - 3 * HOUR, text: '- back up the camera card\n- test the new USB stick\n- dog food\n' }),
    item('C:\\Users\\alex\\Documents\\lease-2026.pdf', { mime: 'application/pdf', size: 1.1 * MB, modified: now - 30 * DAY }),
    item('C:\\Users\\alex\\Downloads\\installer.msi', { mime: 'application/x-msi', size: 84 * MB, modified: now - 5 * HOUR }),
    item('C:\\Users\\alex\\Downloads\\holiday.mp4', { mime: 'video/mp4', size: 1.4 * GB, modified: now - 2 * DAY }),
    pic('C:\\Users\\alex\\Pictures\\wallpaper.jpg', PHOTO_TINTS[2]),
    dir('C:\\Windows\\System32', { modified: now - 2 * DAY }),
    item('C:\\Windows\\explorer.exe', { mime: 'application/x-msdownload', size: 5.2 * MB, readOnly: true }),

    // H: the eBay disk
    dir('H:\\Backups', { modified: now - 1 * DAY }),
    dir('H:\\ISOs', { modified: now - 40 * DAY }),
    item('H:\\ISOs\\debian-13-amd64-netinst.iso', { mime: 'application/x-iso9660-image', size: 0.7 * GB, modified: now - 40 * DAY }),

    // E: a USB stick
    dir('E:\\Music', { modified: now - 3 * DAY }),
    dir('E:\\Backups', { modified: now - 8 * DAY }),
    item('E:\\Music\\mixtape.mp3', { mime: 'audio/mpeg', size: 7.4 * MB }),
    item('E:\\Backups\\phone-2026-10.zip', { mime: 'application/zip', size: 5.9 * GB, modified: now - 8 * DAY }),
    item('E:\\README.txt', { mime: 'text/plain', size: 44, text: 'If found, please return this stick to Alex.\n' }),

    // F: a camera's SD card
    dir('F:\\DCIM', { modified: now - 2 * HOUR }),
    dir('F:\\DCIM\\100CANON', { modified: now - 2 * HOUR }),
    ...shots('F:\\DCIM\\100CANON', 'IMG_', 14, 0),

    // D: a DVD
    dir('D:\\VIDEO_TS', { modified: now - 900 * DAY, readOnly: true }),
    item('D:\\VIDEO_TS\\VTS_01_1.VOB', { mime: 'video/mpeg', size: 1 * GB, modified: now - 900 * DAY, readOnly: true }),
    item('D:\\autorun.inf', { mime: 'text/plain', size: 52, modified: now - 900 * DAY, readOnly: true, text: '[autorun]\nlabel=SUMMER FILMS\n' }),
  ];

  const drive = (letter, extra) => ({
    key: `${DEMO_SERIALS[letter.toLowerCase()]}|${extra.fileSystem}|${extra.total}`,
    path: `${letter}:\\`, label: '', media: true, readOnly: false, vault: false, removable: false,
    serial: DEMO_SERIALS[letter.toLowerCase()], firstSeen: now - 200 * DAY, pluggedIn: now - 6 * DAY, lastTest: null, ...extra,
  });
  const drives = [
    drive('C', { name: 'Local Disk (C:)', kind: 'fixed', fileSystem: 'NTFS', total: 1000 * GB, free: 412 * GB, bus: 'NVMe', model: 'Samsung SSD 990 PRO 1TB', vault: true,
      health: { state: 'ok', powerOnHours: 2210, temperature: 41, passed: true, badSectors: 0, wearPercent: 3, note: '', at: now - 10 * MIN_MS } }),
    drive('H', { name: 'TANK (H:)', label: 'TANK', kind: 'fixed', fileSystem: 'NTFS', total: 12 * TB, free: 3.1 * TB, bus: 'SATA', model: 'WDC WD120EMFZ-11A6JA0', firstSeen: now - 400 * DAY,
      health: { state: 'worn', powerOnHours: 67012, temperature: 38, passed: true, badSectors: 8, wearPercent: null, note: '', at: now - 10 * MIN_MS } }),
    drive('D', { name: 'SUMMER FILMS (D:)', label: 'SUMMER FILMS', kind: 'optical', removable: true, readOnly: true, fileSystem: 'UDF', total: 4.4 * GB, free: 0, bus: 'SATA', model: 'HL-DT-ST DVDRAM GH24NSD1', firstSeen: now - 40 * HOUR, pluggedIn: now - 40 * MIN_MS }),
    drive('E', { name: 'KINGSTON (E:)', label: 'KINGSTON', kind: 'removable', removable: true, fileSystem: 'exFAT', total: 64 * GB, free: 51 * GB, bus: 'USB', model: 'Kingston DataTraveler 3.0', firstSeen: now - 61 * DAY, pluggedIn: now - 2 * HOUR,
      lastTest: { at: now - 9 * DAY, mode: 'write', writeMbps: 38.2, readMbps: 131.4, bytes: 256 * MB } }),
    drive('F', { name: 'EOS_DIGITAL (F:)', label: 'EOS_DIGITAL', kind: 'removable', removable: true, fileSystem: 'FAT32', total: 32 * GB, free: 9.5 * GB, bus: 'SD', model: 'Generic SD/MMC', firstSeen: now - 12 * MIN_MS, pluggedIn: now - 12 * MIN_MS }),
  ];
  const gone = [
    { name: 'BACKUP_2TB (G:)', path: 'G:\\', kind: 'external', total: 2000 * GB, firstSeen: now - 300 * DAY, lastSeen: now - 7 * DAY,
      lastTest: { at: now - 7 * DAY, mode: 'write', writeMbps: 118.6, readMbps: 162.9, bytes: 256 * MB } },
  ];
  return { vault: DEMO_VAULT, separator: '\\', drives, gone, nodes, smartctl: true };
}
