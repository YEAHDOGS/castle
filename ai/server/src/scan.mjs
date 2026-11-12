// Finds model weight files on this machine: the project's models/ folder, the runtime downloads, ComfyUI's own
// models folder, and the Hugging Face hub cache of every user profile. Hub caches keep files as
// snapshots/<rev>/<path> symlinks into blobs/, which break when a repo folder is copied without its blobs;
// those are re-resolved by blob hash across all known caches and otherwise reported as broken.
import { promises as fs } from "node:fs";
import path from "node:path";
import { createHash } from "node:crypto";
import { MODELS_DIR, RUNTIME, COMFY_DIR, LINKED_DIR, hubRoots } from "./config.mjs";

export const WEIGHT_EXT = new Set([".safetensors", ".gguf", ".ckpt", ".pt", ".pth", ".sft"]);
const MIN_BYTES = 5 * 2 ** 20; // skip tokenizer/config-sized files
// Folder names ComfyUI understands, used as a hint for where a file belongs.
export const COMFY_FOLDERS = ["diffusion_models", "unet", "text_encoders", "clip", "vae", "checkpoints", "loras", "audio_encoders", "upscale_models"];
const SKIP_DIRS = new Set([".git", ".venv", "node_modules", "__pycache__", ".locks", "blobs"]);

export function locations() {
  const hubs = hubRoots();
  return [
    { label: "project", dir: MODELS_DIR, kind: "mixed" },
    { label: "runtime", dir: path.join(RUNTIME, "models"), kind: "plain" },
    { label: "comfyui", dir: path.join(COMFY_DIR, "models"), kind: "plain" },
    ...hubs.map((dir) => ({ label: `hf-cache:${userOf(dir)}`, dir, kind: "hub" })),
  ];
}

function userOf(hubDir) {
  const parts = hubDir.split(path.sep);
  const i = parts.findIndex((p) => p === ".cache");
  return i > 0 ? parts[i - 1] : hubDir;
}

const exists = (p) => fs.access(p).then(() => true, () => false);
const shortId = (s) => createHash("sha1").update(s).digest("hex").slice(0, 10);

// Follow a (possibly broken) symlink chain and return the real file, looking up blobs by hash in every cache.
async function resolveFile(p, blobDirs) {
  try {
    const real = await fs.realpath(p);
    const st = await fs.stat(real);
    if (st.isFile()) return { real, size: st.size };
  } catch {}
  let cur = p;
  for (let hop = 0; hop < 8; hop++) {
    let target;
    try { target = await fs.readlink(cur); } catch { break; }
    const hash = path.basename(target);
    for (const b of blobDirs) {
      for (const cand of [path.join(b, hash), path.join(b, hash.slice(0, 2), hash)]) {
        try {
          const real = await fs.realpath(cand);
          const st = await fs.stat(real);
          if (st.isFile()) return { real, size: st.size, recovered: true };
        } catch {}
      }
    }
    cur = path.resolve(path.dirname(cur), target);
  }
  return { real: null, size: null, blob: path.basename(cur) };
}

async function* walk(dir, depth = 0) {
  if (depth > 8) return;
  let entries;
  try { entries = await fs.readdir(dir, { withFileTypes: true }); } catch { return; }
  for (const e of entries) {
    const p = path.join(dir, e.name);
    if (e.isDirectory()) {
      if (SKIP_DIRS.has(e.name) || path.resolve(p) === LINKED_DIR) continue;
      yield* walk(p, depth + 1);
    } else if (e.isFile() || e.isSymbolicLink()) {
      if (WEIGHT_EXT.has(path.extname(e.name).toLowerCase())) yield p;
    }
  }
}

function folderHint(relParts) {
  for (let i = relParts.length - 2; i >= 0; i--) if (COMFY_FOLDERS.includes(relParts[i])) return relParts[i];
  return null;
}

// models--org--name/snapshots/<rev>/...  (prefer the revision refs/main points at)
async function* hubRepoFiles(repoDir) {
  const repo = path.basename(repoDir).replace(/^models--/, "").replace(/--/g, "/");
  const snaps = path.join(repoDir, "snapshots");
  let revs = [];
  try { revs = await fs.readdir(snaps); } catch { return; }
  let main = null;
  try { main = (await fs.readFile(path.join(repoDir, "refs", "main"), "utf8")).trim(); } catch {}
  if (main && revs.includes(main)) revs = [main];
  for (const rev of revs) {
    const base = path.join(snaps, rev);
    for await (const p of walk(base)) yield { p, repo, rev, rel: path.relative(base, p) };
  }
}

async function* locationFiles(loc) {
  let entries = [];
  try { entries = await fs.readdir(loc.dir, { withFileTypes: true }); } catch { return; }
  for (const e of entries) {
    const p = path.join(loc.dir, e.name);
    if (e.isDirectory() && e.name.startsWith("models--")) {
      yield* hubRepoFiles(p);
    } else if (loc.kind !== "hub") {
      if (e.isDirectory()) {
        if (SKIP_DIRS.has(e.name) || path.resolve(p) === LINKED_DIR) continue;
        for await (const f of walk(p)) yield { p: f, repo: null, rev: null, rel: path.relative(loc.dir, f) };
      } else if (WEIGHT_EXT.has(path.extname(e.name).toLowerCase())) {
        yield { p, repo: null, rev: null, rel: e.name };
      }
    }
  }
}

/** Scan every location. Returns { locations, files } with one entry per distinct weight file. */
export async function scanFiles() {
  const locs = locations();
  for (const l of locs) l.exists = await exists(l.dir);
  // Blob stores to search when a snapshot link is broken: every hub cache plus copied repos' own blobs.
  const blobDirs = [];
  for (const l of locs.filter((l) => l.exists)) {
    if (l.kind === "hub") blobDirs.push(path.join(l.dir, "blobs"));
    try {
      for (const e of await fs.readdir(l.dir)) if (e.startsWith("models--")) blobDirs.push(path.join(l.dir, e, "blobs"));
    } catch {}
  }

  const byReal = new Map();
  const broken = [];
  for (const loc of locs.filter((l) => l.exists)) {
    for await (const f of locationFiles(loc)) {
      const r = await resolveFile(f.p, blobDirs);
      const name = path.basename(f.p);
      const parts = f.rel.split(/[\\/]/);
      const rec = {
        name,
        ext: path.extname(name).toLowerCase(),
        repo: f.repo,
        rel: f.rel.replace(/\\/g, "/"),
        folder: folderHint(parts),
        location: loc.label,
        seenAt: f.p,
        path: r.real,
        size: r.size,
        recovered: !!r.recovered,
      };
      if (!r.real) { broken.push({ ...rec, id: shortId(f.p), broken: true, blob: r.blob }); continue; }
      if (r.size < MIN_BYTES) continue;
      if (!byReal.has(r.real)) byReal.set(r.real, { ...rec, id: shortId(r.real), broken: false, alsoAt: [] });
      else byReal.get(r.real).alsoAt.push(f.p);
    }
  }
  const files = [...byReal.values()];
  // A broken copy only matters when no working copy of the same file was found elsewhere.
  const good = new Set(files.map((f) => `${f.repo}|${f.rel}`));
  for (const b of broken) if (!good.has(`${b.repo}|${b.rel}`) && !files.some((f) => f.name === b.name)) files.push(b);
  return { locations: locs, files };
}
