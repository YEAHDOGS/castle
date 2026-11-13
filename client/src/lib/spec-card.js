// The spec card: the box's numbers on one picture, made to be screenshotted and posted.
// Cores, memory, terabytes, how many disks and how long the oldest has been powered on,
// uptime, clones, Theater and Arcade. Drawn here on a canvas; nothing is sent anywhere
// unless the person shares it.
//
// Also the drive-health words the app uses wherever a disk shows its power-on hours.

const W = 1200;
const H = 630; // the shape link previews use, so a shared card is not cropped
const PAD = 64;
const HOURS_PER_YEAR = 8766;
const TB = 1e12; // drives are sold in powers of ten, so they are counted in them here
const GB_BIN = 1024 ** 3;
const DAY_MS = 864e5;
const HOUR_MS = 36e5;
/** From here up a disk's hours are worth showing off. */
export const VETERAN_HOURS = 40_000;

const C = {
  void: '#050508',
  panel: 'rgba(12, 14, 18, 0.9)',
  hairline: 'rgba(255, 255, 255, 0.08)',
  grid: 'rgba(255, 255, 255, 0.035)',
  chiral: '#ff6a00',
  bridge: '#00d7ff',
  white: '#ffffff',
  dim: 'rgba(255, 255, 255, 0.55)',
  faint: 'rgba(255, 255, 255, 0.38)',
};
const SANS = 'Inter, Outfit, system-ui, sans-serif';
const MONO = '"Roboto Mono", ui-monospace, "Cascadia Mono", Menlo, Consolas, monospace';

// ---------------------------------------------------------------- drive health words

/** "67,012 HRS" */
export const fmtHours = (hours) => `${Math.round(hours).toLocaleString('en-US')} HRS`;

/** "7.6 years" of being switched on. */
export const fmtYearsOn = (hours) => {
  const years = hours / HOURS_PER_YEAR;
  return years < 1 ? `${Math.max(1, Math.round(hours / 24))} days` : `${years.toFixed(1)} years`;
};

/**
 * What to say about a disk's health, or null when there is nothing to say.
 * @returns {{ tone: 'bad' | 'warn' | 'quiet', title: string, text: string } | null}
 */
export function healthWords(health) {
  if (!health) return null;
  if (health.state === 'failing') {
    return {
      tone: 'bad',
      title: 'THIS DISK SAYS IT IS FAILING',
      text: 'Its own check failed. Copy what matters to another drive today, not this weekend. Your drives are janky; your data does not have to be.',
    };
  }
  if (health.state === 'worn') {
    const why =
      health.wearPercent >= 100
        ? `It has used ${health.wearPercent}% of its rated life.`
        : `${health.badSectors.toLocaleString('en-US')} bad ${health.badSectors === 1 ? 'sector' : 'sectors'}.`;
    return { tone: 'warn', title: 'WORN', text: `${why} It still works. It is also not getting better. Keep a second copy of anything you would miss.` };
  }
  if (health.state === 'unknown' && health.note) return { tone: 'quiet', title: 'HEALTH UNKNOWN', text: health.note };
  return null;
}

// ---------------------------------------------------------------- the numbers

/** The box's disks in a few numbers, from a drive list (when the box has no /specs). */
export function summarizeDrives(drives = [], smartctl = false) {
  const disks = drives.filter((d) => d.media && (d.kind === 'fixed' || d.kind === 'external'));
  const read = disks.map((d) => d.health).filter((h) => h && !h.note);
  return {
    disks: disks.length,
    total: disks.reduce((sum, d) => sum + (d.total || 0), 0),
    free: disks.reduce((sum, d) => sum + (d.free || 0), 0),
    oldestHours: read.reduce((max, h) => (h.powerOnHours != null && h.powerOnHours > (max ?? -1) ? h.powerOnHours : max), null),
    reporting: read.length,
    failing: read.filter((h) => h.state === 'failing').length,
    worn: read.filter((h) => h.state === 'worn').length,
    smartctl,
  };
}

/** Asks, and never throws: a number that cannot be read is left off the card. */
async function quietly(read) {
  try {
    return await read();
  } catch {
    return null;
  }
}

/**
 * Everything the card shows, from whichever sources are there.
 * @param {{ box: object, storage?: object, clones?: object, theater?: object, arcade?: object }} sources
 */
export async function gatherSpecs({ box, storage, clones, theater, arcade }) {
  const [vitals, specs, computer, cloneList, shelf, systems] = await Promise.all([
    quietly(() => box.system()),
    quietly(() => box.specs?.()),
    quietly(() => storage?.hostDrives?.()),
    quietly(() => clones?.clones()),
    quietly(() => theater?.theater()),
    quietly(() => arcade?.arcadeSystems()),
  ]);
  const drives = specs?.drives || (computer ? summarizeDrives(computer.drives, computer.smartctl) : null);
  return {
    host: vitals?.host || 'castle',
    os: vitals?.os || '',
    processor: specs?.processor || '',
    cores: vitals?.cores ?? null,
    memory: vitals?.memoryTotal ?? null,
    uptime: vitals ? (vitals.uptimeMillis ?? vitals.uptimeSeconds * 1000) : null,
    drives,
    clones: Array.isArray(cloneList) ? cloneList.length : null,
    theater: shelf ? shelf.libraries.reduce((sum, l) => sum + (l.count || 0), 0) : null,
    games: Array.isArray(systems) ? systems.reduce((sum, s) => sum + (s.games || 0), 0) : null,
    at: Date.now(),
  };
}

const fmtUptime = (ms) => {
  const days = Math.floor(ms / DAY_MS);
  const hours = Math.floor((ms % DAY_MS) / HOUR_MS);
  return days ? `${days}D ${String(hours).padStart(2, '0')}H` : `${hours}H ${String(Math.floor((ms % HOUR_MS) / 6e4)).padStart(2, '0')}M`;
};
const fmtTB = (bytes) => (bytes >= TB ? `${(bytes / TB).toFixed(bytes >= 10 * TB ? 0 : 1)}` : `${Math.round(bytes / 1e9)}`);
const tbUnit = (bytes) => (bytes >= TB ? 'TB' : 'GB');
const count = (n) => Number(n).toLocaleString('en-US');

/** The tiles on the card, in reading order. Missing numbers are left out. */
export function specTiles(s) {
  const tiles = [];
  if (s.cores != null) tiles.push({ label: 'CORES', value: count(s.cores) });
  if (s.memory != null) tiles.push({ label: 'MEMORY', value: `${Math.round(s.memory / GB_BIN)}`, unit: 'GB' });
  if (s.drives?.total) tiles.push({ label: 'STORAGE', value: fmtTB(s.drives.total), unit: tbUnit(s.drives.total) });
  if (s.drives) {
    const bad = s.drives.failing ? `${s.drives.failing} FAILING` : s.drives.worn ? `${s.drives.worn} WORN` : '';
    tiles.push({ label: 'DISKS', value: count(s.drives.disks), note: bad, hot: !!s.drives.failing, warn: !!bad });
  }
  if (s.drives?.oldestHours != null) {
    tiles.push({ label: 'OLDEST DISK', value: count(s.drives.oldestHours), unit: 'HRS', note: `${fmtYearsOn(s.drives.oldestHours).toUpperCase()} POWERED ON`, accent: s.drives.oldestHours >= VETERAN_HOURS });
  }
  if (s.uptime != null) tiles.push({ label: 'UPTIME', value: fmtUptime(s.uptime) });
  if (s.clones != null) tiles.push({ label: 'CLONES', value: count(s.clones) });
  if (s.theater) tiles.push({ label: 'THEATER', value: count(s.theater), unit: 'ITEMS' });
  if (s.games) tiles.push({ label: 'ARCADE', value: count(s.games), unit: 'GAMES' });
  return tiles;
}

// ---------------------------------------------------------------- drawing

/** Writes `text` in at most `max` pixels, cutting it with an ellipsis if it does not fit. */
function fit(ctx, text, max) {
  if (ctx.measureText(text).width <= max) return text;
  let cut = text;
  while (cut.length > 1 && ctx.measureText(`${cut}…`).width > max) cut = cut.slice(0, -1);
  return `${cut}…`;
}

function corners(ctx, x, y, w, h, size, color) {
  ctx.strokeStyle = color;
  ctx.lineWidth = 3;
  ctx.beginPath();
  for (const [cx, cy, dx, dy] of [[x, y, 1, 1], [x + w, y, -1, 1], [x, y + h, 1, -1], [x + w, y + h, -1, -1]]) {
    ctx.moveTo(cx + dx * size, cy);
    ctx.lineTo(cx, cy);
    ctx.lineTo(cx, cy + dy * size);
  }
  ctx.stroke();
}

/**
 * Draws the card. Waits for the page's fonts first, so the canvas uses them.
 * @returns {Promise<HTMLCanvasElement>}
 */
export async function drawSpecCard(specs, canvas = document.createElement('canvas')) {
  try {
    await document.fonts?.ready;
  } catch {
    /* system fonts then */
  }
  canvas.width = W;
  canvas.height = H;
  const ctx = canvas.getContext('2d');

  // The ground: near black, a faint grid, a warm glow from one corner and a cool one from the other.
  ctx.fillStyle = C.void;
  ctx.fillRect(0, 0, W, H);
  ctx.strokeStyle = C.grid;
  ctx.lineWidth = 1;
  for (let x = 0; x <= W; x += 40) {
    ctx.beginPath();
    ctx.moveTo(x + 0.5, 0);
    ctx.lineTo(x + 0.5, H);
    ctx.stroke();
  }
  for (let y = 0; y <= H; y += 40) {
    ctx.beginPath();
    ctx.moveTo(0, y + 0.5);
    ctx.lineTo(W, y + 0.5);
    ctx.stroke();
  }
  for (const [x, y, color] of [[0, 0, 'rgba(255, 106, 0, 0.16)'], [W, H, 'rgba(0, 215, 255, 0.12)']]) {
    const glow = ctx.createRadialGradient(x, y, 0, x, y, 620);
    glow.addColorStop(0, color);
    glow.addColorStop(1, 'rgba(0, 0, 0, 0)');
    ctx.fillStyle = glow;
    ctx.fillRect(0, 0, W, H);
  }
  corners(ctx, 24, 24, W - 48, H - 48, 28, C.chiral);

  // Header: the name, and what it runs on.
  ctx.textBaseline = 'alphabetic';
  ctx.fillStyle = C.chiral;
  ctx.font = `700 15px ${MONO}`;
  ctx.fillText('CASTLE · SPEC SHEET', PAD, PAD + 18);
  ctx.fillStyle = C.white;
  ctx.font = `900 54px ${SANS}`;
  ctx.fillText(fit(ctx, specs.host.toUpperCase(), W - 2 * PAD - 260), PAD, PAD + 76);
  ctx.fillStyle = C.dim;
  ctx.font = `500 17px ${MONO}`;
  ctx.fillText(fit(ctx, [specs.processor, specs.os].filter(Boolean).join(' · ').toUpperCase() || 'SELF-HOSTED', W - 2 * PAD), PAD, PAD + 108);
  ctx.textAlign = 'right';
  ctx.fillStyle = C.faint;
  ctx.font = `500 14px ${MONO}`;
  ctx.fillText(new Date(specs.at).toISOString().slice(0, 10), W - PAD, PAD + 18);
  ctx.textAlign = 'left';

  // The tiles: up to five a row, two rows.
  const tiles = specTiles(specs).slice(0, 10);
  const perRow = Math.min(5, Math.max(3, Math.ceil(tiles.length / 2)));
  const gap = 14;
  const top = PAD + 140;
  const tileW = (W - 2 * PAD - gap * (perRow - 1)) / perRow;
  const tileH = 150;
  tiles.forEach((tile, i) => {
    const x = PAD + (i % perRow) * (tileW + gap);
    const y = top + Math.floor(i / perRow) * (tileH + gap);
    ctx.fillStyle = C.panel;
    ctx.fillRect(x, y, tileW, tileH);
    ctx.strokeStyle = tile.hot ? C.chiral : C.hairline;
    ctx.lineWidth = 1;
    ctx.strokeRect(x + 0.5, y + 0.5, tileW - 1, tileH - 1);
    ctx.fillStyle = tile.accent ? C.chiral : C.faint;
    ctx.font = `700 13px ${MONO}`;
    ctx.fillText(tile.label, x + 18, y + 30);

    ctx.font = `900 50px ${MONO}`;
    let size = 50;
    const room = tileW - 36 - (tile.unit ? 8 + tile.unit.length * 11 : 0);
    while (size > 26 && ctx.measureText(tile.value).width > room) {
      size -= 2;
      ctx.font = `900 ${size}px ${MONO}`;
    }
    ctx.fillStyle = tile.accent ? C.chiral : C.white;
    ctx.fillText(tile.value, x + 18, y + 96);
    if (tile.unit) {
      const end = x + 18 + ctx.measureText(tile.value).width + 8;
      ctx.fillStyle = C.faint;
      ctx.font = `700 15px ${MONO}`;
      ctx.fillText(tile.unit, end, y + 96);
    }
    if (tile.note) {
      ctx.fillStyle = tile.warn ? C.chiral : C.bridge;
      ctx.font = `600 12px ${MONO}`;
      ctx.fillText(fit(ctx, tile.note, tileW - 36), x + 18, y + 128);
    }
  });

  // Footer: what it is, deadpan.
  ctx.fillStyle = C.faint;
  ctx.font = `600 14px ${MONO}`;
  ctx.fillText('MY FILES, ON MY BOX. NO ACCOUNT. NO LOGIN WALL. NO MONTHLY FEE.', PAD, H - PAD + 6);
  ctx.textAlign = 'right';
  ctx.fillStyle = C.bridge;
  ctx.fillText('RUNS ON THE PC YOU ALREADY HAVE', W - PAD, H - PAD + 6);
  ctx.textAlign = 'left';
  return canvas;
}

/** The card as a PNG file, named after the box. */
export async function specCardFile(canvas, host) {
  const blob = await new Promise((resolve, reject) => canvas.toBlob((b) => (b ? resolve(b) : reject(new Error('The card could not be made'))), 'image/png'));
  const name = `castle-specs-${(host || 'castle').toLowerCase().replace(/[^a-z0-9]+/g, '-').replace(/^-|-$/g, '') || 'castle'}.png`;
  return new File([blob], name, { type: 'image/png' });
}

/** True when this device can hand the picture to another app (phones, mostly). */
export function canShareFile(file) {
  try {
    return typeof navigator !== 'undefined' && !!navigator.canShare?.({ files: [file] });
  } catch {
    return false;
  }
}

/** Saves the picture through the browser's download. */
export function downloadFile(file) {
  const url = URL.createObjectURL(file);
  const a = document.createElement('a');
  a.href = url;
  a.download = file.name;
  document.body.append(a);
  a.click();
  a.remove();
  setTimeout(() => URL.revokeObjectURL(url), 1000);
}
