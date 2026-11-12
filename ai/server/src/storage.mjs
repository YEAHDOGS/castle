// Where this stack keeps data on disk, how big each place is, and which ones are safe to empty.
// Sizes count each file once even when it is hard-linked into several places (runtime/models/linked).
import { promises as fs } from "node:fs";
import { spawn } from "node:child_process";
import os from "node:os";
import path from "node:path";
import { REPO, RUNTIME, COMFY_DIR, OUTPUTS, MODELS_DIR, LINKED_DIR, IS_WIN, hubRoots } from "./config.mjs";

const CACHE_MS = 60_000;
const home = os.homedir();
const localAppData = process.env.LOCALAPPDATA || path.join(home, "AppData", "Local");
const userOf = (p) => p.split(path.sep)[p.split(path.sep).indexOf("Users") + 1] || "";

/**
 * Every place we know about. `clear` marks folders that are pure caches (they refill themselves on demand), the
 * only ones the app will empty. Model folders are managed per model in Library → Models instead.
 */
function places() {
  const list = [
    { id: "outputs", label: "Generated media", kind: "media", dir: OUTPUTS, about: "Everything you've generated, plus uploads for characters and start frames." },
    { id: "project-models", label: "Project models folder", kind: "models", dir: MODELS_DIR, about: "Models you copied into the project. Hugging Face folders here may just be links into a cache." },
    { id: "linked", label: "ComfyUI model links", kind: "links", dir: LINKED_DIR, about: "Hard links so ComfyUI can see your models. They share space with the originals (counted once)." },
    ...hubRoots().map((dir) => ({ id: `hf-${userOf(dir) || "env"}`, label: `Hugging Face cache (${userOf(dir) || "HF_HOME"})`, kind: "models", dir, about: "Models downloaded with the Hugging Face CLI by this user." })),
    { id: "hf-xet", label: "Hugging Face download cache", kind: "cache", dir: path.join(home, ".cache", "huggingface", "xet"), clear: true, about: "Chunks kept to speed up re-downloads. Safe to clear." },
    { id: "ollama", label: "Ollama models", kind: "models", dir: path.join(process.env.OLLAMA_MODELS || path.join(home, ".ollama", "models")), about: "Chat models (Qwen). Remove individual models in Library → Models." },
    { id: "comfyui", label: "ComfyUI + PyTorch", kind: "runtime", dir: COMFY_DIR, about: "The image/video/audio engine and its Python packages. Reinstall with install.ps1." },
    { id: "comfy-temp", label: "ComfyUI temp + inputs", kind: "cache", dirs: [path.join(COMFY_DIR, "temp"), path.join(COMFY_DIR, "input")], dir: path.join(COMFY_DIR, "temp"), clear: true, about: "Previews and uploaded start frames ComfyUI keeps around. Safe to clear." },
    { id: "database", label: "History database", kind: "data", dir: path.join(RUNTIME, "dogs.db"), about: "Prompts, chats and characters (SQLite)." },
    { id: "app-deps", label: "App build tools (node_modules)", kind: "dev", dirs: [path.join(REPO, "app", "node_modules"), path.join(REPO, "mcp", "comfy-mcp", "node_modules")], dir: path.join(REPO, "app", "node_modules"), about: "Needed to build the app. `npm run app:install` restores them." },
    { id: "app-builds", label: "App builds", kind: "cache", dirs: [path.join(REPO, "app", "dist"), path.join(REPO, "app", "src-tauri", "target")], dir: path.join(REPO, "app", "src-tauri", "target"), clear: true, about: "Rust build output, installers and packaged apps (app/src-tauri/target/). Rebuilt by npm run dist." },
    { id: "uv-cache", label: "Python package cache (uv)", kind: "cache", dir: path.join(IS_WIN ? localAppData : path.join(home, ".cache"), "uv", "cache"), clear: true, about: "Downloaded Python wheels, shared with other projects. Safe to clear; reinstalls download again." },
    { id: "tauri-cache", label: "Installer tool cache", kind: "cache", dir: path.join(localAppData, "tauri"), clear: true, about: "NSIS and WiX downloads used when packaging the desktop app. Safe to clear." },
  ];
  return list.map((p) => ({ ...p, dirs: p.dirs || [p.dir] }));
}

// Walk a tree without following links. `seen` tracks file identities so hard links count once overall.
async function measure(target, seen) {
  let bytes = 0, files = 0, shared = 0;
  async function walk(p, depth) {
    let st;
    try { st = await fs.lstat(p, { bigint: true }); } catch { return; }
    if (st.isSymbolicLink()) return;
    if (st.isDirectory()) {
      if (depth > 40) return;
      let entries = [];
      try { entries = await fs.readdir(p); } catch { return; }
      for (const e of entries) await walk(path.join(p, e), depth + 1);
      return;
    }
    if (!st.isFile()) return;
    files++;
    const key = `${st.dev}:${st.ino}`;
    if (st.nlink > 1n && seen.has(key)) { shared += Number(st.size); return; }
    seen.add(key);
    bytes += Number(st.size);
  }
  await walk(target, 0);
  return { bytes, files, shared };
}

let cache = null;
export async function storageReport({ refresh = false } = {}) {
  if (cache && !refresh && Date.now() - cache.at < CACHE_MS) return cache.value;
  const seen = new Set();
  const out = [];
  // Measure model stores before the link folder, so the links are the ones reported as shared.
  const order = (p) => (p.id === "linked" ? 1 : 0);
  for (const p of places().sort((a, b) => order(a) - order(b))) {
    const parts = await Promise.all(p.dirs.map(async (d) => ({ d, exists: await fs.access(d).then(() => true, () => false) })));
    if (!parts.some((x) => x.exists)) continue;
    let bytes = 0, files = 0, shared = 0;
    for (const { d, exists } of parts) {
      if (!exists) continue;
      const m = await measure(d, seen);
      bytes += m.bytes; files += m.files; shared += m.shared;
    }
    out.push({ id: p.id, label: p.label, kind: p.kind, about: p.about, path: p.dir, paths: parts.filter((x) => x.exists).map((x) => x.d), bytes, files, shared, clearable: !!p.clear });
  }
  out.sort((a, b) => b.bytes - a.bytes);
  let disk = null;
  try { const s = await fs.statfs(REPO); disk = { free: s.bavail * s.bsize, total: s.blocks * s.bsize }; } catch {}
  const value = { places: out, total: out.reduce((s, p) => s + p.bytes, 0), disk, measured: Date.now() };
  cache = { at: Date.now(), value };
  return value;
}

const find = (id) => places().find((p) => p.id === id);

/** Open a known folder in the system file manager (Explorer / Finder). Only listed places can be opened. */
export async function openPlace(id) {
  const p = find(id);
  if (!p) throw Object.assign(new Error("unknown place"), { status: 404 });
  let target = p.dir;
  for (const d of p.dirs) if (await fs.access(d).then(() => true, () => false)) { target = d; break; }
  const st = await fs.stat(target).catch(() => null);
  if (!st) throw Object.assign(new Error("that folder doesn't exist yet"), { status: 404 });
  const [cmd, args] = IS_WIN
    ? ["explorer.exe", st.isDirectory() ? [target] : [`/select,${target}`]]
    : process.platform === "darwin" ? ["open", st.isDirectory() ? [target] : ["-R", target]] : ["xdg-open", [st.isDirectory() ? target : path.dirname(target)]];
  spawn(cmd, args, { detached: true, stdio: "ignore" }).unref();
  return { opened: target };
}

/** Empty a cache folder (only places marked clearable). Returns the bytes freed. */
export async function clearPlace(id) {
  const p = find(id);
  if (!p?.clear) throw Object.assign(new Error("only cache folders can be cleared here"), { status: 400 });
  const seen = new Set();
  let freed = 0;
  const errors = [];
  for (const d of p.dirs) {
    if (!(await fs.access(d).then(() => true, () => false))) continue;
    freed += (await measure(d, seen)).bytes;
    for (const e of await fs.readdir(d).catch(() => [])) {
      try { await fs.rm(path.join(d, e), { recursive: true, force: true }); } catch (err) { errors.push(`${e}: ${err.code || err.message}`); }
    }
  }
  cache = null;
  return { freed, errors };
}
