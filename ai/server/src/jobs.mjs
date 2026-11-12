// Generation jobs. One runs at a time (one GPU); the rest wait in order. Every job is stored in SQLite
// (db.mjs) with its prompt and outputs; jobs still in flight also live in memory for progress updates.
import { promises as fs } from "node:fs";
import path from "node:path";
import { randomUUID } from "node:crypto";
import { EventEmitter } from "node:events";
import { FAMILIES, normalizeParams } from "./families.mjs";
import { getModel, listModels, prepareModel, ensureExtraPaths, describeProblem } from "./models.mjs";
import { startComfy, stopComfy, comfyUp, runGraph, collectOutputs, freeComfy, unloadLlm, nodeTypes, interrupt, uploadImage } from "./comfy.mjs";
import { OUTPUTS, GENERATED, WORKFLOWS_DIR, VRAM_HANDOFF } from "./config.mjs";
import { saveJob, queryJobs, getJobRow, getJobByOutput, deleteJobRow } from "./db.mjs";
import { recordUse } from "./prompts.mjs";

export const events = new EventEmitter();
const live = new Map(); // queued + running jobs

const MIME = { ".png": "image/png", ".jpg": "image/jpeg", ".jpeg": "image/jpeg", ".webp": "image/webp", ".gif": "image/gif",
  ".mp4": "video/mp4", ".webm": "video/webm", ".mkv": "video/x-matroska", ".mp3": "audio/mpeg", ".flac": "audio/flac", ".opus": "audio/ogg", ".wav": "audio/wav" };
export const mimeOf = (f) => MIME[path.extname(f).toLowerCase()] || "application/octet-stream";

const FINISHED = ["done", "error", "cancelled"];
const MAX_QUEUED = 50;

/** Nothing is buffered (each change is written as it happens); kept so the CLI can await it before exiting. */
export const flush = () => Promise.resolve();

function update(job, patch) {
  Object.assign(job, patch);
  events.emit("job", job);
  if (patch.status || patch.outputs) saveJob(job).catch((e) => console.error("[dogs] saving job failed:", e.message));
  if (FINISHED.includes(job.status)) live.delete(job.id);
}

/** Newest first. In-flight jobs come from memory (they carry live progress), the rest from the database. */
export async function listJobs({ limit = 100, kind, q, status } = {}) {
  const rows = await queryJobs({ limit, kind, q, status });
  return rows.map((r) => live.get(r.id) || r);
}
/** Queued + running jobs. */
export const activeJobs = () => live.size;

export async function getJob(id) { return live.get(id) || (await getJobRow(id)); }

/** Remove a job from history; with files: true also delete its output files. */
export async function removeJob(id, { files = false } = {}) {
  const job = await getJob(id);
  if (!job) return null;
  if (!FINISHED.includes(job.status)) throw Object.assign(new Error("cancel the job before deleting it"), { status: 409 });
  if (files) {
    for (const o of job.outputs) {
      const abs = path.resolve(OUTPUTS, o.file);
      if (abs.startsWith(OUTPUTS + path.sep)) await fs.rm(abs, { force: true });
    }
  }
  await deleteJobRow(id);
  return job;
}

const newSeed = () => Math.floor(Math.random() * 2 ** 48);

/** Validate the request and queue it. Throws on bad input (the API turns that into a 400). */
export async function submit(req) {
  if (live.size >= MAX_QUEUED) throw Object.assign(new Error(`the queue is full (${MAX_QUEUED} jobs); try again when some finish`), { status: 429 });
  const prompt = String(req.prompt || "").trim();
  if (!prompt) throw badRequest("prompt is required");
  const model = await pickModel(req);
  const id = randomUUID().slice(0, 12);
  for (const [k, spec] of Object.entries(model.params)) {
    if (spec.type === "image" && req[k]) req = { ...req, [k]: await storeInputImage(req[k], `${id}_${k}`) };
  }
  if (req.image && "start_image" in model.params && !req.start_image) req = { ...req, start_image: await storeInputImage(req.image, `${id}_start_image`) };
  // A start frame sets the video's shape (the model would otherwise crop it): same aspect, the model's default area.
  if (req.start_image && req.width == null && req.height == null && model.params.width && model.params.height) {
    const d = model.params;
    req = { ...req, ...(await fitToImage(req.start_image, d.width, d.height, d.width.default * d.height.default)) };
  }
  const params = normalizeParams(model.params, req);
  const job = {
    id, kind: model.kind, model: model.id, modelLabel: model.label, prompt,
    params, seed: Number.isInteger(req.seed) ? req.seed : newSeed(), status: "queued", progress: null,
    outputs: [], error: null, created: Date.now(), started: null, finished: null, client: req.client || null,
  };
  live.set(job.id, job);
  update(job, { status: "queued" });
  recordUse(prompt, job.kind).catch((e) => console.error("[dogs] saving prompt failed:", e.message));
  serial(() => run(job, model));
  return job;
}

function badRequest(msg) { const e = new Error(msg); e.status = 400; return e; }

const INPUTS = "inputs";
const MAX_INPUT_BYTES = 25 * 2 ** 20;
const IMAGE_EXT = { "image/png": "png", "image/jpeg": "jpg", "image/webp": "webp" };

/**
 * An input image for a job, as a data: URL (uploads) or a path relative to outputs/ (e.g. a Library image).
 * Uploads are saved under outputs/inputs/ so the job can be re-run later. Returns the outputs-relative path.
 */
async function storeInputImage(value, name) {
  if (!value) return "";
  const s = String(value);
  const m = /^data:(image\/(?:png|jpeg|webp));base64,(.+)$/s.exec(s);
  if (m) {
    const buf = Buffer.from(m[2], "base64");
    if (buf.length > MAX_INPUT_BYTES) throw badRequest("image is larger than 25 MB");
    const rel = `${INPUTS}/${name}.${IMAGE_EXT[m[1]]}`;
    await fs.mkdir(path.join(OUTPUTS, INPUTS), { recursive: true });
    await fs.writeFile(path.join(OUTPUTS, rel), buf);
    return rel;
  }
  const rel = s.replace(/\\/g, "/").replace(/^\/?files\//, "");
  const abs = path.resolve(OUTPUTS, rel);
  if (!abs.startsWith(OUTPUTS + path.sep)) throw badRequest("image must be a data: URL or a file under outputs/");
  try { await fs.access(abs); } catch { throw badRequest(`image not found: ${rel}`); }
  return rel;
}

/** Pixel size of a PNG, JPEG or WebP from its header; null if unrecognized. */
export function imageSize(b) {
  if (b.length > 24 && b.readUInt32BE(0) === 0x89504e47) return { width: b.readUInt32BE(16), height: b.readUInt32BE(20) };
  if (b.length > 30 && b.toString("ascii", 0, 4) === "RIFF" && b.toString("ascii", 8, 12) === "WEBP") {
    const chunk = b.toString("ascii", 12, 16);
    if (chunk === "VP8X") return { width: 1 + b.readUIntLE(24, 3), height: 1 + b.readUIntLE(27, 3) };
    if (chunk === "VP8L") { const v = b.readUInt32LE(21); return { width: 1 + (v & 0x3fff), height: 1 + ((v >> 14) & 0x3fff) }; }
    if (chunk === "VP8 ") return { width: b.readUInt16LE(26) & 0x3fff, height: b.readUInt16LE(28) & 0x3fff };
  }
  if (b[0] === 0xff && b[1] === 0xd8) {
    // Walk JPEG segments to the first start-of-frame marker (C0-CF, except C4 DHT, C8 JPG, CC DAC).
    for (let i = 2; i + 9 < b.length;) {
      if (b[i] !== 0xff) { i++; continue; }
      const marker = b[i + 1];
      if (marker >= 0xc0 && marker <= 0xcf && ![0xc4, 0xc8, 0xcc].includes(marker)) return { width: b.readUInt16BE(i + 7), height: b.readUInt16BE(i + 5) };
      i += 2 + b.readUInt16BE(i + 2);
    }
  }
  return null;
}

/**
 * Width/height for a video that starts from this image (outputs-relative path): the image's aspect ratio at
 * about `area` pixels, snapped to the params' step and clamped to their range. {} if the size can't be read.
 */
async function fitToImage(rel, wSpec, hSpec, area) {
  const size = imageSize(await fs.readFile(path.join(OUTPUTS, rel)));
  if (!size?.width || !size?.height) return {};
  const aspect = size.width / size.height;
  const snap = (v, s) => Math.min(s.max, Math.max(s.min, Math.round(v / s.step) * s.step));
  return { width: snap(Math.sqrt(area * aspect), wSpec), height: snap(Math.sqrt(area / aspect), hSpec) };
}

// One-click "Animate": small enough to finish in a few minutes on a 6-8 GB GPU.
const ANIMATE_AREA = 640 * 640;
const ANIMATE_MOTION = "subtle natural motion, gentle slow camera push-in, cinematic";

/**
 * Turn an image into a video with sensible defaults: { image, motion?, model?, seconds?, seed?, ...params }.
 * `image` is a path under outputs/ (a generated image) or a data: URL. The prompt is the motion text plus the
 * prompt the image was made with, when it came from a job here. The video job's params.start_image links it
 * back to the image.
 */
export async function animate(req) {
  if (!req.image) throw badRequest("image is required (a path under outputs/ or a data: URL)");
  const model = req.model ? await pickModel({ model: req.model, kind: "video" }) : await pickVideoModel();
  if (!("start_image" in model.params)) throw badRequest(`${model.label} can't start from an image`);
  const isData = String(req.image).startsWith("data:");
  const rel = await storeInputImage(req.image, `${randomUUID().slice(0, 12)}_animate`);
  const source = isData ? null : await getJobByOutput(rel);
  const motion = String(req.motion || "").trim();
  const prompt = [motion || ANIMATE_MOTION, source?.prompt].filter(Boolean).join(". ");
  const { image, motion: _m, ...rest } = req;
  const size = rest.width == null && rest.height == null && model.params.width
    ? await fitToImage(rel, model.params.width, model.params.height, Math.min(ANIMATE_AREA, model.params.width.default * model.params.height.default))
    : {};
  return submit({ ...rest, ...size, model: model.id, prompt, start_image: rel });
}

// Wan 2.2 TI2V first (better motion), then any other ready video model that takes a start frame.
const VIDEO_PREFERENCE = ["wan2.2-ti2v-5b", "ltx-video-2b"];
async function pickVideoModel() {
  const ready = (await listModels()).models.filter((m) => m.kind === "video" && m.status === "ready" && "start_image" in m.params);
  const rank = (m) => { const i = VIDEO_PREFERENCE.indexOf(m.family); return i < 0 ? VIDEO_PREFERENCE.length : i; };
  const m = ready.sort((a, b) => rank(a) - rank(b))[0];
  if (!m) throw badRequest("no video model that can animate an image is installed: run `dogs download wan2.2-ti2v-5b` (or ltx-video-2b)");
  return m;
}

async function pickModel(req) {
  if (req.model) {
    const m = await getModel(req.model);
    if (!m) throw badRequest(`unknown model "${req.model}" (see GET /api/models)`);
    if (m.status !== "ready") throw badRequest(`${m.label} is not ready: ${describeProblem(m)}`);
    if (req.kind && m.kind !== req.kind) throw badRequest(`${m.label} is a ${m.kind} model, not ${req.kind}`);
    return m;
  }
  if (!req.kind) throw badRequest("give either model or kind (image | video | audio)");
  const m = (await listModels()).models.find((x) => x.kind === req.kind && x.status === "ready");
  if (!m) throw badRequest(`no ready ${req.kind} model installed (see GET /api/models)`);
  return m;
}

let queue = Promise.resolve();
const serial = (fn) => (queue = queue.then(fn, fn));

let restartChecked = false;
async function run(job, model) {
  if (job.status === "cancelled") return;
  update(job, { status: "running", started: Date.now(), progress: { stage: "preparing" } });
  try {
    // The linked-models folder must be in extra_model_paths.yaml before ComfyUI starts; restart once if it was added late.
    const changed = await ensureExtraPaths();
    if (changed && !restartChecked && (await comfyUp())) { update(job, { progress: { stage: "restarting ComfyUI" } }); await stopComfy(); }
    restartChecked = true;
    const files = await prepareModel(model);
    update(job, { progress: { stage: "starting ComfyUI" } });
    await startComfy();
    const graph = await buildGraph(job, model, files);
    const have = await nodeTypes();
    const missingNodes = [...new Set(Object.values(graph).map((n) => n.class_type))].filter((t) => !have.has(t));
    if (missingNodes.length) throw new Error(`ComfyUI is missing node types ${missingNodes.join(", ")}. ` +
      (missingNodes.includes("UnetLoaderGGUF") ? "GGUF weights need the ComfyUI-GGUF custom node: run `dogs comfy add-node gguf`." : "Update ComfyUI or install the custom node."));
    update(job, { progress: { stage: "loading model" } });
    await unloadLlm();
    const items = await runGraph(graph, {
      onQueued: (pid) => (job.promptId = pid),
      onProgress: (p) => { if (job.status === "running") update(job, { progress: { stage: p.max ? "sampling" : "running", ...p } }); },
    });
    if (job.status === "cancelled") return;
    const outputs = (await collectOutputs(items)).map((f) => ({ file: f, url: `/files/${f}`, mime: mimeOf(f) }));
    if (!outputs.length) throw new Error("the workflow finished but produced no output files");
    update(job, { status: "done", outputs, progress: null, finished: Date.now() });
  } catch (e) {
    if (job.status !== "cancelled") update(job, { status: "error", error: String(e.message || e), progress: null, finished: Date.now() });
  } finally {
    if (VRAM_HANDOFF) await freeComfy().catch(() => {});
  }
}

async function buildGraph(job, model, files) {
  const prefix = `${GENERATED}/${model.kind}/${(model.family || "dogs").replace(/[^\w.-]/g, "_")}`;
  if (model.custom) {
    const raw = JSON.parse(await fs.readFile(path.join(WORKFLOWS_DIR, model.file), "utf8"));
    delete raw.dogs;
    const vars = { prompt: job.prompt, seed: job.seed, prefix, ...job.params };
    const fill = (v) => {
      if (typeof v === "string") {
        const whole = v.match(/^\{\{(\w+)\}\}$/);
        if (whole && whole[1] in vars) return vars[whole[1]];
        return v.replace(/\{\{(\w+)\}\}/g, (m, k) => (k in vars ? String(vars[k]) : m));
      }
      if (Array.isArray(v)) return v.map(fill);
      if (v && typeof v === "object") return Object.fromEntries(Object.entries(v).map(([k, x]) => [k, fill(x)]));
      return v;
    };
    return fill(raw);
  }
  const fam = FAMILIES.find((f) => f.id === model.family);
  // Input images go to ComfyUI's input folder; the graph gets the name ComfyUI assigned.
  let startImage = "";
  if (job.params.start_image) {
    const abs = path.join(OUTPUTS, job.params.start_image);
    startImage = await uploadImage(await fs.readFile(abs), path.basename(abs));
  }
  return fam.graph({ files, prompt: job.prompt, seed: job.seed, prefix, ...job.params, startImage });
}

export async function cancel(id) {
  const job = await getJob(id);
  if (!job) return null;
  if (job.status === "queued") update(job, { status: "cancelled", finished: Date.now() });
  else if (job.status === "running") { update(job, { status: "cancelled", finished: Date.now(), progress: null }); await interrupt(job.promptId); }
  return job;
}

/** Wait until a job is finished (done, error or cancelled). */
export function waitFor(job, timeoutMs = 60 * 60 * 1000) {
  if (["done", "error", "cancelled"].includes(job.status)) return Promise.resolve(job);
  return new Promise((resolve) => {
    const t = setTimeout(() => { events.off("job", on); resolve(job); }, timeoutMs);
    const on = (j) => { if (j.id === job.id && ["done", "error", "cancelled"].includes(j.status)) { clearTimeout(t); events.off("job", on); resolve(j); } };
    events.on("job", on);
  });
}
