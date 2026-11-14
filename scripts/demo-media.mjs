#!/usr/bin/env node
// Builds media/: the images the demo's Arcade and Theater show, every one public domain or under a free
// licence (Creative Commons, or the free-software licence of the game it shows). Upload what is in the folder to
// data.wearedogs.net/castle/ (arcade/, theater/ and the credits); the demo loads its images from there (MEDIA_BASE in
// app/src/lib/demo-shell.js).
//
//   node scripts/demo-media.mjs           fetch what is missing and rebuild media/
//
// What is fetched, from where and under which licence is listed below and written to media/CREDITS.md and
// media/credits.json, which the licences ask for. Downloads are kept in app/data/demo-media-cache, so a rerun
// only fetches new entries. Needs Node 22+ and ffmpeg on PATH.
import { execFileSync } from "node:child_process";
import { existsSync, mkdirSync, rmSync, statSync, writeFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const OUT = path.join(ROOT, "media");
const CACHE = path.join(ROOT, "app", "data", "demo-media-cache");
// Wikimedia asks automated clients to name themselves and to go slowly.
const HEADERS = { "user-agent": "castle-demo-media/1.0 (sample data for an open-source media app)" };
const API_GAP_MS = 3000;
const DOWNLOAD_GAP_MS = 2500;
const WIKI_BATCH = 40;
// Wikimedia only makes thumbnails in its standard widths (330, 500, 960, 1280, ...).
const WIKI_WIDTH = 960;

// Sizes the app shows them at, doubled for sharp screens where it matters.
const POSTER = [400, 600];
const SQUARE = [400, 400];
const STILL = [640, 360];
const PHOTO = [800, 600];

const LIBRETRO = "https://thumbnails.libretro.com";
const ARCHIVE = "https://archive.org/download";

// ---- What to fetch -------------------------------------------------------------------------------------------
// wiki: a file on Wikipedia or Commons (its licence is read from the file page and must be a free one).
// url: anything else, with its licence stated here. frame: a still taken from a public-domain video at `at` s.

const film = (slug, wiki) => ({ out: `theater/posters/${slug}.jpg`, wiki, size: POSTER });
const FILMS = [
  film("metropolis", "File:Metropolis_(German_three-sheet_poster).jpg"),
  film("nosferatu", "File:Nosferatu_poster_(Albin_Grau,_1922)_1.jpg"),
  film("the-general", "File:The_General_(1926)_-_Movie_Poster_2.png"),
  film("sherlock-jr", "File:Sherlock_jr_poster.jpg"),
  film("the-kid", "File:The_Kid_(1921)_poster.jpg"),
  film("the-gold-rush", "File:Gold_rush_poster.jpg"),
  film("safety-last", "File:Safety_last_poster.jpg"),
  film("the-phantom-of-the-opera", "File:Phantom_of_the_opera_1925_poster.jpg"),
  film("the-cabinet-of-dr-caligari", "File:Das_Cabinet_des_Dr._Caligari.JPG"),
  film("his-girl-friday", "File:His_Girl_Friday_(1940_poster)_crop.jpg"),
  film("detour", "File:Detour_(poster).jpg"),
  film("doa", "File:D.O.A._(1950_poster).jpg"),
  film("plan-9-from-outer-space", "File:Plan_9_Alternative_poster.jpg"),
  film("the-little-shop-of-horrors", "File:The_Little_Shop_of_Horrors_(1960)_-_Half-Sheet_poster.webp"),
  film("carnival-of-souls", "File:Carnival_of_Souls_(1962_pressbook_cover).jpg"),
  film("charade", "File:Charade_(1963_poster).jpg"),
  film("night-of-the-living-dead", "File:Night_Of_The_Living_Dead_(1968)_-_Poster.jpg"),
  film("elephants-dream", "File:ElephantsDreamPoster.jpg"),
  film("big-buck-bunny", "File:Big_buck_bunny_poster_big.jpg"),
  film("sita-sings-the-blues", "File:Sita_STB_Poster.jpg"),
  film("sintel", "File:Sintel_poster.jpg"),
  film("tears-of-steel", "File:Tos-poster.png"),
  film("cosmos-laundromat", "File:CosmosLaundromatPoster.jpg"),
  film("spring", "File:Spring2019PillarPosterBlender.jpg"),
];

const still = (slug, wiki) => ({ out: `theater/stills/${slug}.jpg`, wiki, size: STILL });
const frame = (slug, wiki, at) => ({ out: `theater/stills/${slug}.jpg`, wiki, at, size: STILL });
const SHOWS = [
  { out: "theater/posters/superman.jpg", wiki: "File:Supermanshorttitle.PNG", size: POSTER },
  still("superman-s1e1", "File:Supermanshorttitle.PNG"),
  still("superman-s1e2", "File:Title_card_from_The_Mechanical_Monsters.jpg"),
  still("superman-s1e3", "File:Title_card_from_Billion_Dollar_Limited.jpg"),
  still("superman-s1e4", "File:Title_card_from_The_Arctic_Giant.jpg"),
  still("superman-s1e5", "File:Title_card_from_The_Bulleteers.jpg"),
  still("superman-s1e6", "File:Title_card_from_The_Magnetic_Telescope.jpg"),
  still("superman-s1e7", "File:Title_card_from_Electric_Earthquake.jpg"),
  still("superman-s1e8", "File:Title_card_from_Volcano.jpg"),
  still("superman-s1e9", "File:Terroronthemidway1.JPG"),
  still("superman-s2e2", "File:Showdown_title_screen.JPG"),
  still("superman-s2e4", "File:Title_card_from_Destruction_Inc.jpg"),
  still("superman-s2e5", "File:Themummystrikes.JPG"),
  still("superman-s2e7", "File:The_underground_world_title.JPG"),
  still("superman-s2e8", "File:Secret_Agent_title.PNG"),
  { out: "theater/posters/popeye-the-sailor.jpg", wiki: "File:Popeye_Meets_Sinbad.PNG", size: POSTER },
  still("popeye-the-sailor-s1e1", "File:Popeye_Meets_Sinbad.PNG"),
  frame("popeye-the-sailor-s1e2", "File:Popeye_The_Sailor_Meets_Ali_Baba's_Forty_Thieves_(1937).webm", 20),
  { out: "theater/posters/the-beverly-hillbillies.jpg", wiki: "File:Beverly_Hillbillies_Episode_02_Getting_Settled.ogv", at: 15, size: POSTER },
  frame("the-beverly-hillbillies-s1e2", "File:Beverly_Hillbillies_Episode_02_Getting_Settled.ogv", 240),
  frame("the-beverly-hillbillies-s1e7", "File:Beverly_Hillbillies_Episode_07_The_Servants.ogv", 240),
  frame("the-beverly-hillbillies-s1e10", "File:Beverly_Hillbillies_Episode_10_Pygmalion_And_Elly.ogv", 240),
  frame("the-beverly-hillbillies-s1e17", "File:Beverly_Hillbillies_Episode_17_Jeds_Dilemma.ogv", 240),
  frame("the-beverly-hillbillies-s1e18", "File:Beverly_Hillbillies_Episode_18_Jed_Saves_The_Drysdales_Marriage.ogv", 240),
  frame("the-beverly-hillbillies-s1e25", "File:Beverly_Hillbillies_Episode_25_The_Family_Tree.ogv", 240),
  frame("the-beverly-hillbillies-s2e3", "File:Beverly_Hillbillies_Episode_39_Grannys_Garden.ogv", 240),
  frame("the-beverly-hillbillies-s2e5", "File:Beverly_Hillbillies_Episode_41_The_Clampett_Look.ogv", 240),
  frame("the-beverly-hillbillies-s2e7", "File:Beverly_Hillbillies_Episode_43_Chickadee_Returns.ogv", 240),
];

const album = (slug, id, file, artist, title, licence) => ({
  out: `theater/covers/${slug}.jpg`, url: `${ARCHIVE}/${id}/${encodeURIComponent(file)}`, size: SQUARE,
  credit: { title: `${artist} - ${title} (cover)`, author: artist, licence, page: `https://archive.org/details/${id}` },
});
const BY25 = { name: "CC BY 2.5", url: "https://creativecommons.org/licenses/by/2.5/" };
const BYSA3 = { name: "CC BY-SA 3.0", url: "https://creativecommons.org/licenses/by-sa/3.0/" };
const BYSA4 = { name: "CC BY-SA 4.0", url: "https://creativecommons.org/licenses/by-sa/4.0/" };
const ALBUMS = [
  album("the-animal-economy", "USR-019", "USR-019large.jpg", "MonoCulture", "The Animal Economy", BY25),
  album("the-four-seasons", "JOAT022", "01-frontcovermemphis.jpg", "Memphis", "The Four Seasons", BYSA3),
  album("experiments-at-the-edge-of-time", "mt007", "mt007_artwork_front_400x400.jpg", "Softspace", "Experiments at the Edge of Time", BY25),
  album("paramo", "MNF041_Callejas--Paramo", "MNF041_Front.jpg", "Callejas", "Páramo", BYSA4),
];

// Photo albums: free photos found by a Commons search, the first `count` with a free licence.
const PHOTO_SEARCHES = [
  { folder: "beach-2026", query: "beach sunset waves filetype:bitmap", count: 12 },
  { folder: "birthday", query: "birthday cake candles filetype:bitmap", count: 8 },
];

// Arcade: freely licensed homebrew and libretro's own games. Screenshots and box art come from libretro's
// thumbnail server (what RetroArch shows); they picture games whose code and art are under the licence given.
const retro = (system, slug, name, kinds, licence, page) =>
  Object.entries(kinds).map(([kind, file]) => ({
    out: `arcade/${system.slug}/${kind}/${slug}.png`,
    url: `${LIBRETRO}/${encodeURIComponent(system.name)}/${{ boxart: "Named_Boxarts", snap: "Named_Snaps", title: "Named_Titles" }[kind]}/${encodeURIComponent(file)}.png`,
    pixel: system.pixel && kind !== "boxart",
    credit: { title: `${name} (${kind === "boxart" ? "box art" : kind === "snap" ? "screenshot" : "title screen"})`, author: page.author, licence, page: page.url },
  }));
const NES = { slug: "nes", name: "Nintendo - Nintendo Entertainment System", pixel: true };
const GB = { slug: "gb", name: "Nintendo - Game Boy", pixel: true };
const DOOM = { slug: "doom", name: "DOOM", pixel: false };
const DINO = { slug: "dinothawr", name: "Dinothawr", pixel: false };
const BOOM = { slug: "mrboom", name: "MrBoom", pixel: false };
const GPL3 = { name: "GPL-3.0", url: "https://www.gnu.org/licenses/gpl-3.0.html" };
const ZLIB = { name: "zlib", url: "https://opensource.org/license/zlib" };
const WTFPL = { name: "WTFPL", url: "http://www.wtfpl.net/" };
const MIT = { name: "MIT", url: "https://opensource.org/license/mit" };
const BSD3 = { name: "BSD-3-Clause", url: "https://opensource.org/license/bsd-3-clause" };
const CCBY4_MIT = { name: "MIT (code), CC BY 4.0 (art)", url: "https://github.com/SimonLarsen/tobutobugirl" };
const CCBYSA3_GPL3 = { name: "GPL-3.0 (code), CC BY-SA 3.0 (art)", url: "https://github.com/libretro/Dinothawr" };
const both = (file) => ({ snap: file, title: file });
const GAMES = [
  ...retro(NES, "super-tilt-bro", "Super Tilt Bro.", both("Super Tilt Bro. (World) (v2.0-beta6) (Aftermarket) (Unl)"), WTFPL, { author: "Sylvain Gadrat", url: "https://github.com/sgadrat/super-tilt-bro" }),
  ...retro(NES, "nova-the-squirrel", "Nova the Squirrel", both("Nova the Squirrel (World) (v1.06) (Aftermarket) (Unl)"), GPL3, { author: "NovaSquirrel", url: "https://github.com/NovaSquirrel/NovaTheSquirrel" }),
  ...retro(NES, "thwaite", "Thwaite", both("Thwaite (World) (v0.04) (Aftermarket) (Unl)"), GPL3, { author: "Damian Yerrick", url: "https://github.com/pinobatch/thwaite-nes" }),
  ...retro(NES, "concentration-room", "Concentration Room", both("Concentration Room (World) (v0.02) (Aftermarket) (Unl)"), GPL3, { author: "Damian Yerrick", url: "https://github.com/pinobatch/croom-nes" }),
  ...retro(NES, "robotfindskitten", "robotfindskitten", both("robotfindskitten (World) (v0.9.106) (Damian Yerrick) (Aftermarket) (Unl)"), ZLIB, { author: "Damian Yerrick", url: "https://github.com/pinobatch/rfk-nes" }),
  ...retro(NES, "double-action-blaster-guys", "DABG: Double Action Blaster Guys", both("DABG - Double Action Blaster Guys (World) (Aftermarket) (Unl)"), ZLIB, { author: "NovaSquirrel", url: "https://github.com/NovaSquirrel/DABG" }),
  ...retro(GB, "tobu-tobu-girl", "Tobu Tobu Girl", { title: "Tobu Tobu Girl (World) (Aftermarket) (Unl)" }, CCBY4_MIT, { author: "Tangram Games", url: "https://github.com/SimonLarsen/tobutobugirl" }),
  ...retro(DOOM, "freedoom-phase-1", "Freedoom: Phase 1", both("Freedoom - Phase 1 (v0.11.3)"), BSD3, { author: "The Freedoom project", url: "https://freedoom.github.io/" }),
  ...retro(DOOM, "freedoom-phase-2", "Freedoom: Phase 2", both("Freedoom - Phase 2 (v0.11.3)"), BSD3, { author: "The Freedoom project", url: "https://freedoom.github.io/" }),
  ...retro(DOOM, "freedm", "FreeDM", { title: "Freedoom - FreeDM (v0.11.3)" }, BSD3, { author: "The Freedoom project", url: "https://freedoom.github.io/" }),
  ...retro(DINO, "dinothawr", "Dinothawr", { boxart: "Dinothawr", snap: "Dinothawr" }, CCBYSA3_GPL3, { author: "Agnes Heyer, Hans-Kristian Arntzen", url: "https://github.com/libretro/Dinothawr" }),
  ...retro(BOOM, "mr-boom", "Mr. Boom", { boxart: "Mr.Boom", snap: "Mr.Boom" }, MIT, { author: "Remdy Software", url: "https://github.com/libretro/mrboom-libretro" }),
];

// ---- Fetching ------------------------------------------------------------------------------------------------

const sleep = (ms) => new Promise((done) => setTimeout(done, ms));
const strip = (html) => String(html || "").replace(/<[^>]*>/g, "").replace(/\s+/g, " ").trim();
const FREE = /^(public domain|pd|cc0|cc[ -]by(-sa)?[ -]\d|cc by(-sa)? \d|attribution|gfdl)/i;

async function json(url, tries = 4) {
  for (let i = 0; ; i++) {
    const res = await fetch(url, { headers: HEADERS });
    const text = await res.text();
    try {
      return JSON.parse(text);
    } catch {
      if (i >= tries) throw new Error(`${url}: ${text.slice(0, 120)}`);
      await sleep(API_GAP_MS * 5 * (i + 1)); // throttled: back off
    }
  }
}

/** File pages to { url (original), thumb (WIKI_WIDTH wide), page, author, licence }. */
async function resolveWiki(titles) {
  const found = new Map();
  const unique = [...new Set(titles)];
  for (let i = 0; i < unique.length; i += WIKI_BATCH) {
    if (i) await sleep(API_GAP_MS);
    const batch = unique.slice(i, i + WIKI_BATCH);
    const params = new URLSearchParams({
      action: "query", format: "json", formatversion: "2", prop: "imageinfo", titles: batch.join("|"),
      iiprop: "url|size|extmetadata", iiurlwidth: String(WIKI_WIDTH), iiextmetadatafilter: "Artist|LicenseShortName|LicenseUrl|ObjectName",
    });
    const r = await json(`https://en.wikipedia.org/w/api.php?${params}`);
    const asked = new Map(batch.map((t) => [t.replace(/_/g, " "), t]));
    for (const n of r.query.normalized || []) asked.set(n.to, asked.get(n.from.replace(/_/g, " ")) || n.from);
    for (const p of r.query.pages) {
      const ii = p.imageinfo?.[0];
      const key = asked.get(p.title) || p.title;
      if (!ii) { found.set(key, null); continue; }
      const meta = ii.extmetadata || {};
      found.set(key, {
        url: ii.url, thumb: ii.thumburl || ii.url, page: ii.descriptionurl,
        author: strip(meta.Artist?.value) || "Unknown",
        licence: { name: strip(meta.LicenseShortName?.value) || "Unknown", url: meta.LicenseUrl?.value || ii.descriptionurl },
        title: strip(meta.ObjectName?.value) || p.title.replace(/^File:/, ""),
      });
    }
  }
  return found;
}

/** The first `count` freely licensed bitmaps a Commons search finds. */
async function searchCommons(query, count) {
  const params = new URLSearchParams({
    action: "query", format: "json", formatversion: "2", generator: "search", gsrnamespace: "6", gsrsearch: query, gsrlimit: "40",
    prop: "imageinfo", iiprop: "url|size|extmetadata", iiurlwidth: String(WIKI_WIDTH), iiextmetadatafilter: "Artist|LicenseShortName|LicenseUrl|ObjectName",
  });
  const r = await json(`https://commons.wikimedia.org/w/api.php?${params}`);
  return (r.query?.pages || [])
    .sort((a, b) => a.index - b.index)
    .map((p) => ({ p, ii: p.imageinfo?.[0] }))
    .filter(({ ii }) => ii && ii.width >= 800 && ii.width > ii.height && FREE.test(strip(ii.extmetadata?.LicenseShortName?.value)))
    .slice(0, count)
    .map(({ p, ii }) => ({
      url: ii.url, thumb: ii.thumburl || ii.url, page: ii.descriptionurl,
      author: strip(ii.extmetadata.Artist?.value) || "Unknown",
      licence: { name: strip(ii.extmetadata.LicenseShortName.value), url: ii.extmetadata.LicenseUrl?.value || ii.descriptionurl },
      title: strip(ii.extmetadata.ObjectName?.value) || p.title.replace(/^File:/, ""),
    }));
}

let lastDownload = 0;
async function download(url, file) {
  if (existsSync(file) && statSync(file).size > 0) return file;
  await sleep(Math.max(0, lastDownload + DOWNLOAD_GAP_MS - Date.now()));
  lastDownload = Date.now();
  for (let i = 0; ; i++) {
    const res = await fetch(url, { headers: HEADERS });
    if (res.ok) {
      mkdirSync(path.dirname(file), { recursive: true });
      writeFileSync(file, Buffer.from(await res.arrayBuffer()));
      return file;
    }
    if (i >= 5 || (res.status !== 429 && res.status < 500)) throw new Error(`${url}: HTTP ${res.status}`);
    await sleep(15000 * (i + 1)); // throttled: back off

  }
}

function ffmpeg(args) {
  execFileSync("ffmpeg", ["-hide_banner", "-loglevel", "error", "-y", ...args], { stdio: ["ignore", "ignore", "inherit"] });
}

function dims(file) {
  const out = execFileSync("ffprobe", ["-v", "error", "-select_streams", "v:0", "-show_entries", "stream=width,height", "-of", "csv=p=0", file], { encoding: "utf8" });
  const [w, h] = out.trim().split(",").map(Number);
  return { w, h };
}

/**
 * Fits a picture to w x h. One close to that shape is cropped to fill it; one of another shape (a landscape
 * poster for a tall frame) is shown whole over a blurred, darkened copy of itself.
 */
function fit(src, dst, [w, h], { pixel = false } = {}) {
  mkdirSync(path.dirname(dst), { recursive: true });
  const input = ["-i", src];
  const quality = dst.endsWith(".jpg") ? ["-q:v", "3"] : [];
  if (pixel) {
    // Pixel art stays pixel art: doubled with nearest-neighbour, never smoothed or cropped.
    ffmpeg([...input, "-vf", "scale=iw*2:ih*2:flags=neighbor", "-frames:v", "1", dst]);
    return;
  }
  if (!w) {
    ffmpeg([...input, "-vf", "scale='min(512,iw)':-2:flags=lanczos", "-frames:v", "1", dst]);
    return;
  }
  const probe = dims(src);
  const close = Math.abs(probe.w / probe.h - w / h) / (w / h) < 0.25;
  const cover = `scale=${w}:${h}:force_original_aspect_ratio=increase:flags=lanczos,crop=${w}:${h}`;
  if (close) {
    ffmpeg([...input, "-vf", cover, ...quality, "-frames:v", "1", dst]);
    return;
  }
  const graph = `[0:v]split[a][b];[a]${cover},boxblur=24:2,eq=brightness=-0.18[bg];[b]scale=${w}:${h}:force_original_aspect_ratio=decrease:flags=lanczos[fg];[bg][fg]overlay=(W-w)/2:(H-h)/2`;
  ffmpeg([...input, "-filter_complex", graph, ...quality, "-frames:v", "1", dst]);
}

/** Commons' thumbnail of a video at `at` seconds, about as wide as the video (a standard width). */
function videoStill(original, at) {
  const u = new URL(original);
  u.search = "";
  const [, wiki, hash1, hash2, name] = u.pathname.match(/^(\/wikipedia\/commons)\/(\w)\/(\w\w)\/(.+)$/) || [];
  if (!name) throw new Error(`${original}: not a Commons file`);
  const width = /\.ogv$/i.test(name) ? 330 : 1280;
  u.pathname = `${wiki}/thumb/${hash1}/${hash2}/${name}/${width}px-seek%3D${at}-${name}.jpg`;
  return u.toString();
}

const cacheName = (url) => {
  const base = decodeURIComponent(url.split("/").pop().split("?")[0]).replace(/[^\w.-]+/g, "_").slice(-90);
  let n = 0;
  for (const c of url) n = (n * 31 + c.charCodeAt(0)) >>> 0;
  return `${n.toString(36)}-${base}`;
};

async function main() {
  const wanted = [...FILMS, ...SHOWS, ...ALBUMS, ...GAMES];
  console.log(`Resolving ${wanted.filter((e) => e.wiki).length} Wikipedia and Commons files...`);
  const wiki = await resolveWiki(wanted.filter((e) => e.wiki).map((e) => e.wiki));
  for (const search of PHOTO_SEARCHES) {
    await sleep(API_GAP_MS);
    const photos = await searchCommons(search.query, search.count);
    if (photos.length < search.count) console.warn(`  only ${photos.length} free photos for "${search.query}"`);
    photos.forEach((photo, i) => {
      const key = `search:${search.folder}:${i}`;
      wiki.set(key, photo);
      wanted.push({ out: `theater/photos/${search.folder}/${String(i + 1).padStart(2, "0")}.jpg`, wiki: key, size: PHOTO });
    });
  }

  rmSync(OUT, { recursive: true, force: true });
  const credits = [];
  const failed = [];
  for (const entry of wanted) {
    try {
      let source = entry.credit ? { ...entry.credit, url: entry.url } : null;
      let from = entry.url;
      if (entry.wiki) {
        const file = wiki.get(entry.wiki);
        if (!file) throw new Error(`${entry.wiki} was not found`);
        if (!FREE.test(file.licence.name)) throw new Error(`${entry.wiki} is ${file.licence.name}, not a free licence`);
        source = { title: file.title, author: file.author, licence: file.licence, page: file.page, url: file.url };
        // A still from a video is the thumbnail Commons makes of it at that second.
        from = entry.at == null ? file.thumb : videoStill(file.url, entry.at);
      }
      const src = await download(from, path.join(CACHE, cacheName(from)));
      fit(src, path.join(OUT, entry.out), entry.size || [0, 0], { pixel: !!entry.pixel });
      credits.push({ file: entry.out, ...source, ...(entry.at != null ? { note: `still at ${entry.at} s` } : {}) });
      process.stdout.write(".");
    } catch (e) {
      failed.push(`${entry.out}: ${e.message}`);
      process.stdout.write("x");
    }
  }
  console.log();

  credits.sort((a, b) => a.file.localeCompare(b.file));
  writeFileSync(path.join(OUT, "credits.json"), JSON.stringify(credits, null, 2) + "\n");
  const rows = credits.map((c) => `| \`${c.file}\` | [${c.title.replace(/\|/g, "/")}](${c.page}) | ${c.author.replace(/\|/g, "/")} | [${c.licence.name}](${c.licence.url}) |${c.note ? ` ${c.note}` : ""}`);
  writeFileSync(
    path.join(OUT, "CREDITS.md"),
    [
      "# Castle demo media",
      "",
      "Images the Castle demo's Arcade and Theater show. Every one is in the public domain or under a free licence:",
      "Creative Commons for films, shows, music and photos, and the free-software licence of the game for the Arcade's",
      "screenshots and box art. Built by `scripts/demo-media.mjs`; served from data.wearedogs.net/castle.",
      "",
      "| File | Source | Author | Licence |",
      "|---|---|---|---|",
      ...rows,
      "",
    ].join("\n")
  );
  console.log(`${credits.length} images in ${path.relative(ROOT, OUT)}/`);
  if (failed.length) {
    console.log(`${failed.length} failed:\n  ${failed.join("\n  ")}`);
    process.exitCode = 1;
  }
}

await main();
