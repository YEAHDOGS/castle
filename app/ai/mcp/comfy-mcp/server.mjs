#!/usr/bin/env node
// MCP server: exposes FLUX.3 Image Klein (via a local ComfyUI) to opencode as tools.
// Tools: comfy_generate_image, comfy_edit_image, comfy_status, comfy_free_vram.
import { McpServer } from "@modelcontextprotocol/sdk/server/mcp.js";
import { StdioServerTransport } from "@modelcontextprotocol/sdk/server/stdio.js";
import { z } from "zod";
import { spawn } from "node:child_process";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { randomUUID } from "node:crypto";
import { textToImage, imageEdit, MODEL_FILES } from "./workflows.mjs";

const HERE = path.dirname(fileURLToPath(import.meta.url));
const COMFY_URL = (process.env.COMFY_URL || "http://127.0.0.1:8188").replace(/\/$/, "");
const COMFY_AUTOSTART = process.env.COMFY_AUTOSTART || path.resolve(HERE, "../../scripts/start-comfyui.ps1");
const OLLAMA_URL = (process.env.OLLAMA_URL || "http://127.0.0.1:11434").replace(/\/$/, "");
const VRAM_HANDOFF = process.env.VRAM_HANDOFF !== "0";
const OUTPUTS = path.resolve(process.env.DOGS_AI_OUTPUTS || path.resolve(HERE, "../../outputs"));
const CLIENT_ID = randomUUID();

const log = (...a) => console.error("[comfy-mcp]", ...a); // stdout is the MCP channel

async function comfy(pathname, init) {
  const res = await fetch(COMFY_URL + pathname, init);
  if (!res.ok) throw new Error(`ComfyUI ${pathname} -> HTTP ${res.status}: ${await res.text()}`);
  return res;
}

async function comfyUp() {
  try { await comfy("/system_stats", { signal: AbortSignal.timeout(3000) }); return true; } catch { return false; }
}

let starting = null;
async function ensureComfy() {
  if (await comfyUp()) return;
  if (!starting) {
    log("ComfyUI not reachable, starting via", COMFY_AUTOSTART);
    starting = new Promise((resolve, reject) => {
      const p = spawn("powershell.exe", ["-NoProfile", "-ExecutionPolicy", "Bypass", "-File", COMFY_AUTOSTART],
        { stdio: ["ignore", "ignore", "pipe"], windowsHide: true });
      let err = "";
      p.stderr.on("data", (d) => (err += d));
      p.on("exit", (code) => (code === 0 ? resolve() : reject(new Error(`start-comfyui.ps1 exited ${code}: ${err.trim()}`))));
    }).finally(() => (starting = null));
  }
  await starting;
  if (!(await comfyUp())) throw new Error(`ComfyUI still not reachable at ${COMFY_URL}`);
}

// Qwen and FLUX do not both fit in 6 GB of VRAM: unload every loaded Ollama model before sampling
// (Ollama reloads on the next chat turn, from the OS file cache).
async function unloadLlm() {
  if (!VRAM_HANDOFF) return;
  try {
    const { models = [] } = await (await fetch(`${OLLAMA_URL}/api/ps`, { signal: AbortSignal.timeout(5000) })).json();
    for (const m of models) {
      await fetch(`${OLLAMA_URL}/api/generate`, {
        method: "POST", headers: { "content-type": "application/json" },
        body: JSON.stringify({ model: m.name, keep_alive: 0 }), signal: AbortSignal.timeout(15000),
      });
    }
  } catch (e) { log("ollama unload skipped:", e.message); }
}

async function freeComfy() {
  await comfy("/free", { method: "POST", headers: { "content-type": "application/json" },
    body: JSON.stringify({ unload_models: true, free_memory: true }) });
}

async function uploadImage(file) {
  const abs = path.resolve(file);
  const form = new FormData();
  form.append("image", new Blob([await readFile(abs)]), `mcp_${Date.now()}_${path.basename(abs)}`);
  form.append("overwrite", "true");
  const { name, subfolder } = await (await comfy("/upload/image", { method: "POST", body: form })).json();
  return subfolder ? `${subfolder}/${name}` : name;
}

async function runGraph(graph, timeoutMs = 15 * 60 * 1000) {
  const { prompt_id, node_errors } = await (await comfy("/prompt", {
    method: "POST", headers: { "content-type": "application/json" },
    body: JSON.stringify({ prompt: graph, client_id: CLIENT_ID }),
  })).json();
  if (node_errors && Object.keys(node_errors).length) throw new Error("Workflow rejected: " + JSON.stringify(node_errors));
  const t0 = Date.now();
  while (Date.now() - t0 < timeoutMs) {
    await new Promise((r) => setTimeout(r, 1000));
    const h = (await (await comfy(`/history/${prompt_id}`)).json())[prompt_id];
    if (!h) continue;
    if (h.status?.status_str === "error") {
      const msg = (h.status.messages || []).find(([t]) => t === "execution_error")?.[1];
      throw new Error("ComfyUI execution error: " + (msg ? `${msg.node_type}: ${msg.exception_message}` : JSON.stringify(h.status)));
    }
    if (h.status?.completed) {
      return { images: Object.values(h.outputs).flatMap((o) => o.images || []), seconds: (Date.now() - t0) / 1000 };
    }
  }
  throw new Error(`Timed out after ${timeoutMs / 1000}s waiting for ComfyUI prompt ${prompt_id}`);
}

// Every generated file lives in outputs/generated/image (ComfyUI writes it there); save_to gets a copy as well.
const GENERATED_IMAGES = path.join(OUTPUTS, "generated", "image");
const comfyPrefix = (name) => `generated/image/${name}`;

async function saveImages(images, saveTo) {
  const dir = path.resolve(saveTo || GENERATED_IMAGES);
  await mkdir(dir, { recursive: true });
  const out = [];
  for (const img of images) {
    const q = new URLSearchParams({ filename: img.filename, subfolder: img.subfolder || "", type: img.type || "output" });
    const buf = Buffer.from(await (await comfy(`/view?${q}`)).arrayBuffer());
    const dest = path.join(dir, img.filename);
    await writeFile(dest, buf);
    out.push(dest);
  }
  return out;
}

// One generation at a time: the GPU is the bottleneck and handoff/free must not interleave.
let queue = Promise.resolve();
const serial = (fn) => (queue = queue.then(fn, fn));

async function generate(graph, saveTo, seed) {
  return serial(async () => {
    await ensureComfy();
    await unloadLlm();
    try {
      const { images, seconds } = await runGraph(graph);
      const files = await saveImages(images, saveTo);
      return { content: [{ type: "text", text: JSON.stringify({ files, seed, seconds: Math.round(seconds) }, null, 2) }] };
    } finally {
      if (VRAM_HANDOFF) await freeComfy().catch((e) => log("free failed:", e.message));
    }
  });
}

const fail = (e) => ({ isError: true, content: [{ type: "text", text: String(e?.message || e) }] });
const dim = z.number().int().min(256).max(2048).multipleOf(16);
const newSeed = () => Math.floor(Math.random() * 2 ** 48);

const server = new McpServer({ name: "comfy", version: "0.1.0" });

server.registerTool("generate_image", {
  description: "Generate PNG image(s) from a text prompt with FLUX.3 Image Klein (distilled, local ComfyUI). Returns saved file paths.",
  inputSchema: {
    prompt: z.string().min(1).describe("Detailed English caption of the image. Quote any text to render."),
    width: dim.default(1024).describe("Multiple of 16. 1024 square, 1344x768 landscape, 768x1344 portrait."),
    height: dim.default(1024),
    steps: z.number().int().min(4).max(16).default(16).describe("4-16; 8 for drafts."),
    count: z.number().int().min(1).max(4).default(1).describe("Number of images (batch)."),
    seed: z.number().int().min(0).optional().describe("Omit for random."),
    save_to: z.string().optional().describe("Folder to save into (e.g. ./assets). Default: dogs-ai/outputs/generated/image."),
    filename_prefix: z.string().regex(/^[\w-]+$/).default("flux3"),
  },
}, async (a) => {
  try {
    const seed = a.seed ?? newSeed();
    return await generate(textToImage({ ...a, seed, filename_prefix: comfyPrefix(a.filename_prefix) }), a.save_to, seed);
  } catch (e) { return fail(e); }
});

server.registerTool("edit_image", {
  description: "Edit or combine existing image(s) with FLUX.3 Image Klein. Refer to inputs as <ref_image_0>, <ref_image_1>... in the prompt.",
  inputSchema: {
    prompt: z.string().min(1).describe("Full description of the desired result with the edit folded in."),
    images: z.array(z.string()).min(1).max(4).describe("Paths of input images, in <ref_image_N> order."),
    width: dim.optional().describe("Output size; omit to follow the first reference (~1 MP)."),
    height: dim.optional(),
    steps: z.number().int().min(4).max(16).default(16),
    seed: z.number().int().min(0).optional(),
    save_to: z.string().optional(),
    filename_prefix: z.string().regex(/^[\w-]+$/).default("flux3_edit"),
  },
}, async (a) => {
  try {
    const seed = a.seed ?? newSeed();
    const uploaded = [];
    for (const f of a.images) uploaded.push(await serial(async () => { await ensureComfy(); return uploadImage(f); }));
    return await generate(imageEdit({ ...a, images: uploaded, seed, filename_prefix: comfyPrefix(a.filename_prefix) }), a.save_to, seed);
  } catch (e) { return fail(e); }
});

server.registerTool("status", {
  description: "Report whether ComfyUI is up, GPU/VRAM state, queue length, and whether the FLUX model files are installed.",
  inputSchema: {},
}, async () => {
  try {
    const up = await comfyUp();
    const info = { comfy_url: COMFY_URL, comfy_up: up, outputs: OUTPUTS, model_files: MODEL_FILES };
    if (!up) info.note = "ComfyUI is not running; it starts automatically on the first generate/edit call. Installed files can only be checked while it runs.";
    if (up) {
      const s = await (await comfy("/system_stats")).json();
      const gb = (b) => Math.round((b / 2 ** 30) * 10) / 10;
      info.comfyui_version = s.system.comfyui_version;
      info.ram_free_gb = gb(s.system.ram_free);
      info.gpus = s.devices.map((d) => ({ name: d.name, vram_total_gb: gb(d.vram_total), vram_free_gb: gb(d.vram_free) }));
      info.queue = await (await comfy("/queue")).json().then((q) => ({ running: q.queue_running.length, pending: q.queue_pending.length }));
      const lists = {};
      for (const folder of ["diffusion_models", "text_encoders", "vae"]) lists[folder] = await (await comfy(`/models/${folder}`)).json();
      info.missing_model_files = Object.entries(MODEL_FILES).filter(([k, f]) => !lists[k].includes(f)).map(([, f]) => f);
    }
    return { content: [{ type: "text", text: JSON.stringify(info, null, 2) }] };
  } catch (e) { return fail(e); }
});

server.registerTool("free_vram", {
  description: "Unload FLUX models from the GPU in ComfyUI.",
  inputSchema: {},
}, async () => {
  try { await freeComfy(); return { content: [{ type: "text", text: "ComfyUI models unloaded." }] }; }
  catch (e) { return fail(e); }
});

await server.connect(new StdioServerTransport());
log(`ready (ComfyUI ${COMFY_URL}, outputs ${OUTPUTS})`);
