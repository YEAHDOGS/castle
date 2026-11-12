// Turns scanned weight files into selectable models (a family + its main weight file + companion files),
// and makes the chosen files visible to ComfyUI by hard-linking them into runtime/models/linked/<folder>/.
import { promises as fs } from "node:fs";
import path from "node:path";
import { FAMILIES, familyInfo, downloadInfo } from "./families.mjs";
import { scanFiles } from "./scan.mjs";
import { LINKED_DIR, EXTRA_PATHS_YAML, RUNTIME, WORKFLOWS_DIR, REPO } from "./config.mjs";

const fwd = (p) => p.replace(/\\/g, "/");

function pickCompanion(files, role) {
  const cands = files.filter((f) => role.match.test(f.name));
  // Working files first, then the one in the expected folder, then the biggest (usually the higher precision).
  cands.sort((a, b) => (a.broken - b.broken) || ((b.folder === role.folder) - (a.folder === role.folder)) || ((b.size || 0) - (a.size || 0)));
  return cands[0] || null;
}

const fileView = (f) => f && {
  id: f.id, name: f.name, size: f.size, path: f.path, seenAt: f.seenAt, alsoAt: f.alsoAt || [], location: f.location, repo: f.repo,
  broken: f.broken, recovered: f.recovered,
};

async function customWorkflows() {
  let names = [];
  try { names = (await fs.readdir(WORKFLOWS_DIR)).filter((n) => n.endsWith(".json")); } catch { return []; }
  const out = [];
  for (const n of names) {
    try {
      const graph = JSON.parse(await fs.readFile(path.join(WORKFLOWS_DIR, n), "utf8"));
      const meta = graph.dogs || {};
      if (!["image", "video", "audio"].includes(meta.kind)) continue;
      const id = `workflow:${path.basename(n, ".json")}`;
      out.push({
        id, family: "workflow", kind: meta.kind, label: meta.label || path.basename(n, ".json"),
        blurb: meta.blurb || `Custom ComfyUI workflow (workflows/${n})`, status: "ready", custom: true,
        file: n, files: {}, missing: [], requiresNodes: [], params: meta.params || {}, promptLabel: meta.promptLabel || "Prompt",
      });
    } catch {}
  }
  return out;
}

let cache = null;
/** All models, ready or not, plus the scanned locations and the weight files no family claimed. */
export async function listModels({ refresh = false } = {}) {
  if (cache && !refresh && Date.now() - cache.at < 30_000) return cache.value;
  const { locations, files } = await scanFiles();
  const models = [];
  const claimed = new Set();
  for (const fam of FAMILIES) {
    const mains = files.filter((f) => fam.files.model.match.test(f.name));
    for (const main of mains) {
      claimed.add(main.id);
      const roles = { model: main };
      for (const [role, spec] of Object.entries(fam.files)) {
        if (role === "model") continue;
        roles[role] = pickCompanion(files, spec);
        if (roles[role]) claimed.add(roles[role].id);
      }
      const missing = Object.keys(fam.files).filter((r) => !roles[r]);
      const broken = Object.keys(fam.files).filter((r) => roles[r]?.broken);
      const requiresNodes = fam.nodesFor ? fam.nodesFor(Object.fromEntries(Object.entries(roles).map(([r, f]) => [r, f?.name || ""]))) : [];
      const status = missing.length ? "incomplete" : broken.length ? "broken" : "ready";
      models.push({
        id: `${fam.id}:${main.name}`, family: fam.id, kind: fam.kind,
        label: mains.length > 1 ? `${fam.label} (${main.name})` : fam.label,
        blurb: fam.blurb, status, missing, broken, requiresNodes,
        files: Object.fromEntries(Object.entries(roles).map(([r, f]) => [r, fileView(f)])),
        params: fam.params, promptLabel: fam.promptLabel || "Prompt",
        // What a download would still fetch: only the missing or broken files.
        download: downloadInfo(fam, [...missing, ...broken]),
      });
    }
  }
  models.push(...(await customWorkflows()));
  for (const f of files) {
    // Companions that matched a family with no main file still count as claimed (they are not "unknown").
    if (FAMILIES.some((fam) => Object.values(fam.files).some((s) => s.match.test(f.name)))) claimed.add(f.id);
  }
  // Ready models first; within that, the order of FAMILIES (FLUX leads), so FLUX is the default pick and shows first.
  const order = { ready: 0, broken: 1, incomplete: 2 };
  const famIndex = (m) => { const i = FAMILIES.findIndex((f) => f.id === m.family); return i < 0 ? FAMILIES.length : i; };
  models.sort((a, b) => (order[a.status] - order[b.status]) || (famIndex(a) - famIndex(b)) || a.label.localeCompare(b.label));
  // Disk use of every distinct weight file found (each real file counted once, even if seen in several places).
  const present = files.filter((f) => !f.broken);
  const byLocation = {};
  for (const f of present) byLocation[f.location] = (byLocation[f.location] || 0) + f.size;
  let disk = null;
  try { const s = await fs.statfs(REPO); disk = { free: s.bavail * s.bsize, total: s.blocks * s.bsize }; } catch {}
  const value = {
    storage: { bytes: present.reduce((s, f) => s + f.size, 0), files: present.length, byLocation, disk },
    models,
    unrecognized: files.filter((f) => !claimed.has(f.id)).map(fileView),
    locations: locations.map(({ label, dir, exists }) => ({ label, dir, exists })),
    families: FAMILIES.map(familyInfo),
  };
  cache = { at: Date.now(), value };
  return value;
}

export async function getModel(id) {
  const { models } = await listModels();
  return models.find((m) => m.id === id) || null;
}

/** Make a file visible to ComfyUI under <folder>/<name>. Hard link (no extra disk), else symlink, else copy if allowed. */
export async function linkFile(file, folder, { copy = false } = {}) {
  const dir = path.join(LINKED_DIR, folder);
  await fs.mkdir(dir, { recursive: true });
  const dest = path.join(dir, file.name);
  try {
    const st = await fs.stat(dest);
    const src = await fs.stat(file.path);
    if (st.ino && st.ino === src.ino) return file.name;
    if (st.size === src.size) return file.name;
    await fs.rm(dest); // a different file with the same name: replace it
  } catch {}
  const errors = [];
  try { await fs.link(file.path, dest); return file.name; } catch (e) { errors.push(`hardlink: ${e.code || e.message}`); }
  try { await fs.symlink(file.path, dest, "file"); return file.name; } catch (e) { errors.push(`symlink: ${e.code || e.message}`); }
  if (copy) { await fs.copyFile(file.path, dest); return file.name; }
  const err = new Error(`Could not make ${file.name} visible to ComfyUI (${errors.join(", ")}). It is probably on another drive: ` +
    `run "dogs models import" to copy it, or move it into the project's models/ folder.`);
  err.code = "LINK_FAILED";
  throw err;
}

/** Link every file of a ready model. Returns { role: comfyFileName }. */
export async function prepareModel(model, opts) {
  if (model.custom) return {};
  if (model.status !== "ready") throw new Error(`${model.label} is ${model.status}: ${describeProblem(model)}`);
  const fam = FAMILIES.find((f) => f.id === model.family);
  const names = {};
  for (const [role, spec] of Object.entries(fam.files)) names[role] = await linkFile(model.files[role], spec.folder, opts);
  return names;
}

export function describeProblem(m) {
  if (m.missing?.length) return `missing ${m.missing.join(", ")}. Run: dogs download ${m.family}`;
  if (m.broken?.length) return `the ${m.broken.join(", ")} file is a broken Hugging Face cache link (its blob was not copied). ` +
    `Copy the repo's blobs/ folder too, or re-download with: dogs download ${m.family}`;
  return "";
}

/**
 * Delete weight files the scanner found, by id. Removes the real file, every cache symlink / copy that pointed at
 * it, and its hard links in runtime/models/linked (otherwise the space would not be freed). Ids not from the
 * current scan are refused, so nothing outside the scanned model files can be deleted this way.
 * @returns {Promise<{ deleted: string[], freed: number, errors: string[] }>}
 */
export async function deleteFiles(ids) {
  const { files, locations } = await scanFiles();
  // Only ever delete inside the scanned model folders (after resolving links), so a planted symlink that
  // points elsewhere can't turn a model cleanup into deleting an arbitrary file.
  const roots = [];
  for (const l of locations.filter((x) => x.exists)) roots.push(await fs.realpath(l.dir).catch(() => l.dir));
  roots.push(await fs.realpath(LINKED_DIR).catch(() => LINKED_DIR));
  const contained = async (p) => {
    const real = await fs.realpath(path.dirname(p)).then((d) => path.join(d, path.basename(p)), () => null);
    return !!real && roots.some((r) => real === r || real.startsWith(r + path.sep));
  };
  const byId = new Map(files.map((f) => [f.id, f]));
  const out = { deleted: [], freed: 0, errors: [] };
  for (const id of ids) {
    const f = byId.get(id);
    if (!f) { out.errors.push(`${id}: not a scanned model file`); continue; }
    const targets = new Set([f.path, f.seenAt, ...(f.alsoAt || [])].filter(Boolean));
    for (const folder of await fs.readdir(LINKED_DIR).catch(() => [])) targets.add(path.join(LINKED_DIR, folder, f.name));
    let realGone = !f.path;
    for (const t of targets) {
      try {
        if (!(await contained(t))) { out.errors.push(`${t}: outside the model folders, not deleted`); continue; }
        const st = await fs.lstat(t);
        // A linked-dir entry with this name but different content belongs to another model: leave it.
        if (t.startsWith(LINKED_DIR) && f.size && st.size !== f.size && !st.isSymbolicLink()) continue;
        await fs.rm(t);
        if (t === f.path) realGone = true;
        out.deleted.push(t);
      } catch (e) {
        if (e.code !== "ENOENT") out.errors.push(`${t}: ${e.code || e.message}`);
      }
    }
    if (realGone && f.size) out.freed += f.size;
  }
  cache = null;
  return out;
}

/** Make sure ComfyUI's extra_model_paths.yaml includes the linked folder. Returns true when the file changed. */
export async function ensureExtraPaths() {
  let yaml = "";
  try { yaml = await fs.readFile(EXTRA_PATHS_YAML, "utf8"); } catch {}
  if (/^dogs_linked:/m.test(yaml)) return false;
  const block = [
    "dogs_linked:",
    `  base_path: ${fwd(LINKED_DIR)}`,
    ...["diffusion_models", "text_encoders", "vae", "checkpoints", "loras"].map((f) => `  ${f}: ${f}`),
    "",
  ].join("\n");
  await fs.mkdir(RUNTIME, { recursive: true });
  await fs.writeFile(EXTRA_PATHS_YAML, (yaml && !yaml.endsWith("\n") ? yaml + "\n" : yaml) + block);
  for (const f of ["diffusion_models", "text_encoders", "vae", "checkpoints", "loras"]) await fs.mkdir(path.join(LINKED_DIR, f), { recursive: true });
  return true;
}
