#!/usr/bin/env node
// dogs: command line for the local generation stack. Talks to a running `dogs serve` when there is one,
// otherwise does the work in-process.
import path from "node:path";
import { promises as fs } from "node:fs";
import { spawn } from "node:child_process";
import { API_PORT, API_KEY, OUTPUTS, REPO, MODELS_DIR } from "../src/config.mjs";

const HELP = `dogs - local image / video / audio generation (ComfyUI)

Usage:
  dogs serve [--host 0.0.0.0] [--port 8787]   Start the API (and the app at http://host:port/)
  dogs app                                    Open the desktop app
  dogs models [--kind image|video|audio] [--json]
                                              List models found on this machine and what is missing
  dogs where                                  Show every folder that is scanned for models
  dogs llm [--json]                           Local LLMs (Ollama): size, context window, loaded state
  dogs download [family | org/repo] [files..] [--all]
                                              Download with the Hugging Face CLI. No args: list families with sizes.
                                              A family fetches only the files you are missing (--all for every file)
  dogs hf search [words] [--kind image|video|audio] [--sort downloads|likes|trending|modified] [--limit n]
                                              Search Hugging Face models
  dogs hf files <org/repo>                    Weight files in a repo, with sizes and links
  dogs hf <anything else>                     Passed through to the hf CLI (hf auth login, hf cache ls, ...)
  dogs gen <image|video|audio> "<prompt>" [--model <id>] [--<param> <value> ...] [--seed n] [--out dir]
                                              Generate and print the output file paths
  dogs gen video "<prompt>" --image photo.png Image to video: animate photo.png (it becomes the first frame)
  dogs animate <image> ["motion prompt"] [--seconds n] [--model <id>] [--out dir]
                                              One-step image to video: picks the video model, matches the
                                              image's shape, and reuses its prompt if it was generated here
  dogs jobs                                   Recent generations
  dogs comfy start|stop|restart|status        Control the ComfyUI server
  dogs comfy add-node gguf                    Install the ComfyUI-GGUF node (for .gguf weights)
  dogs models import <model id>               Copy a model's files in (when they are on another drive)

Shortcuts:
  dogs image|video|audio "<prompt>"           same as dogs gen <kind> (also img, pic, vid, movie, sound, music)
  dl/get/pull = download   g/make/create = gen   m/ls/list = models   s/start = serve   j/history = jobs
  search/find = hf search  open/ui/desktop = app  chat/ollama = llm   paths = where

Env: DOGS_HOST, DOGS_PORT, DOGS_API_KEY, DOGS_ORIGINS, COMFY_URL, DOGS_MODELS, VRAM_HANDOFF=0
`;

// Alternative names for commands and kinds. `dogs image "..."` is short for `dogs gen image "..."`.
const COMMAND_ALIASES = {
  dl: "download", get: "download", pull: "download", install: "download",
  g: "gen", generate: "gen", make: "gen", create: "gen",
  m: "models", ls: "models", list: "models",
  s: "serve", server: "serve", start: "serve", api: "serve",
  j: "jobs", history: "jobs", h: "jobs",
  i2v: "animate", anim: "animate",
  llms: "llm", chat: "llm", ollama: "llm",
  huggingface: "hf", hub: "hf",
  search: "hf search", find: "hf search",
  open: "app", desktop: "app", ui: "app",
  paths: "where", locations: "where",
};
const KIND_ALIASES = {
  image: "image", img: "image", pic: "image", picture: "image", photo: "image", i: "image",
  video: "video", vid: "video", movie: "video", clip: "video", v: "video",
  audio: "audio", sound: "audio", music: "audio", song: "audio", sfx: "audio", a: "audio",
};
const HF_ALIASES = { find: "search", s: "search", ls: "search", list: "search", f: "files", dl: "download", get: "download" };

function resolveArgs(args) {
  let [first, ...more] = args;
  const lower = first?.toLowerCase();
  if (lower && COMMAND_ALIASES[lower]) [first, ...more] = [...COMMAND_ALIASES[lower].split(" "), ...more];
  else if (lower && KIND_ALIASES[lower] && lower.length > 1) [first, ...more] = ["gen", KIND_ALIASES[lower], ...more];
  if (first === "gen" && more[0] && KIND_ALIASES[more[0].toLowerCase()]) more[0] = KIND_ALIASES[more[0].toLowerCase()];
  if (first === "hf" && more[0] && HF_ALIASES[more[0].toLowerCase()]) more[0] = HF_ALIASES[more[0].toLowerCase()];
  return [first, ...more];
}

const argv = resolveArgs(process.argv.slice(2));
const [cmd, ...rest] = argv;

function flags(args) {
  const pos = [], opt = {};
  for (let i = 0; i < args.length; i++) {
    const a = args[i];
    if (a.startsWith("--")) {
      const [k, inline] = a.slice(2).split("=", 2);
      if (inline !== undefined) opt[k] = inline;
      else if (i + 1 < args.length && !args[i + 1].startsWith("--")) opt[k] = args[++i];
      else opt[k] = true;
    } else pos.push(a);
  }
  return { pos, opt };
}

const API = `http://127.0.0.1:${API_PORT}`;
const headers = { "content-type": "application/json", ...(API_KEY ? { authorization: `Bearer ${API_KEY}` } : {}) };
async function apiUp() {
  try { return (await fetch(`${API}/api/health`, { headers, signal: AbortSignal.timeout(1500) })).ok; } catch { return false; }
}

const gb = (b) => (b == null ? "?" : `${(b / 2 ** 30).toFixed(1)} GB`);
const STATUS = { ready: "\x1b[32mready\x1b[0m", incomplete: "\x1b[33mincomplete\x1b[0m", broken: "\x1b[31mbroken\x1b[0m" };

async function cmdModels(opt) {
  const { listModels } = await import("../src/models.mjs");
  const r = await listModels({ refresh: true });
  const models = opt.kind ? r.models.filter((m) => m.kind === opt.kind) : r.models;
  if (opt.json) return console.log(JSON.stringify({ ...r, models }, null, 2));
  if (!models.length) {
    console.log("No models found.\n");
    return printWhere(r);
  }
  for (const m of models) {
    console.log(`${m.kind.padEnd(6)} ${STATUS[m.status] || m.status}  ${m.label}\n       id: ${m.id}`);
    for (const [role, f] of Object.entries(m.files)) {
      if (f) console.log(`       ${role.padEnd(13)} ${f.name} (${gb(f.size)}, ${f.location}${f.broken ? ", BROKEN LINK" : ""})`);
      else console.log(`       ${role.padEnd(13)} \x1b[33mmissing\x1b[0m`);
    }
    if (m.status !== "ready" && m.download) console.log(`       -> dogs download ${m.family}   (${gb(m.download.size)} to fetch)`);
    if (m.requiresNodes?.length) console.log(`       needs ComfyUI node ${m.requiresNodes.join(", ")} -> dogs comfy add-node gguf`);
  }
  const have = new Set(r.models.map((m) => m.family));
  const more = r.families.filter((f) => !have.has(f.id) && (!opt.kind || f.kind === opt.kind));
  if (more.length) {
    console.log("\nNot installed (dogs download <id>):");
    for (const f of more) console.log(`  ${f.kind.padEnd(6)} ${f.id.padEnd(18)} ${gb(f.download.size).padStart(8)}  ${f.label}  ${f.download.repos.map((r) => r.url).join(" ")}`);
  }
  if (r.unrecognized.length) {
    console.log("\nWeight files no model family recognizes (add a workflow in workflows/ to use them):");
    for (const f of r.unrecognized) console.log(`  ${f.name} (${gb(f.size)}, ${f.location})`);
  }
}

function printWhere(r) {
  console.log("Scanned folders:");
  for (const l of r.locations) console.log(`  ${l.exists ? "\x1b[32m✓\x1b[0m" : "\x1b[2m·\x1b[0m"} ${l.label.padEnd(18)} ${l.dir}`);
  console.log(`
Models downloaded with the Hugging Face CLI land in <user profile>\\.cache\\huggingface\\hub of whichever
account ran the download (yours, or an admin's). Every profile's cache is scanned, so they show up here
as long as this account can read that folder. You can also cut/paste a models--<org>--<name> folder
(include its blobs/ folder) or plain .safetensors files into:
  ${MODELS_DIR}

Download one:  dogs download flux3-klein      (or any org/repo:  dogs download Comfy-Org/Qwen-Image-2.1 <file>)`);
}

async function cmdGen(pos, opt) {
  const [kind, ...words] = pos;
  if (!["image", "video", "audio"].includes(kind) || !words.length) throw new Error('usage: dogs gen <image|video|audio> "<prompt>"');
  const { model, seed, out, ...params } = opt;
  for (const k of Object.keys(params)) if (/^-?\d+(\.\d+)?$/.test(params[k])) params[k] = Number(params[k]);
  // --image / --start_image take a local file; it is sent inline so this also works against a remote server.
  for (const k of ["image", "start_image"]) {
    if (typeof params[k] !== "string") continue;
    const ext = path.extname(params[k]).toLowerCase();
    const mime = { ".png": "image/png", ".jpg": "image/jpeg", ".jpeg": "image/jpeg", ".webp": "image/webp" }[ext];
    if (!mime) throw new Error(`--${k} must be a .png, .jpg or .webp file`);
    params[k] = `data:${mime};base64,${(await fs.readFile(params[k])).toString("base64")}`;
  }
  const body = { kind, prompt: words.join(" "), model, seed: seed !== undefined ? Number(seed) : undefined, ...params, client: "cli" };
  await runJob("/api/generate", body, (jobs) => jobs.submit(body), out);
}

async function cmdAnimate(pos, opt) {
  const [file, ...words] = pos;
  if (!file) throw new Error('usage: dogs animate <image> ["motion prompt"]');
  const { model, seed, out, ...params } = opt;
  for (const k of Object.keys(params)) if (/^-?\d+(\.\d+)?$/.test(params[k])) params[k] = Number(params[k]);
  // An image under outputs/ goes by path, so the server finds the prompt it was made with; anything else inline.
  const abs = path.resolve(file);
  let image = path.relative(OUTPUTS, abs).replace(/\\/g, "/");
  if (image.startsWith("..") || path.isAbsolute(image)) {
    const mime = { ".png": "image/png", ".jpg": "image/jpeg", ".jpeg": "image/jpeg", ".webp": "image/webp" }[path.extname(abs).toLowerCase()];
    if (!mime) throw new Error("the image must be a .png, .jpg or .webp file");
    image = `data:${mime};base64,${(await fs.readFile(abs)).toString("base64")}`;
  } else await fs.access(abs);
  const body = { image, motion: words.join(" "), model, seed: seed !== undefined ? Number(seed) : undefined, ...params, client: "cli" };
  await runJob("/api/animate", body, (jobs) => jobs.animate(body), out);
}

/** Queue through a running `dogs serve` (so the app sees the job), or run in-process; print the output paths. */
async function runJob(endpoint, body, local, out) {
  let job;
  if (await apiUp()) {
    const res = await fetch(`${API}${endpoint}`, { method: "POST", headers, body: JSON.stringify(body) });
    job = await res.json();
    if (!res.ok) throw new Error(job.error);
    process.stderr.write(`queued ${job.id} on ${job.modelLabel} (via ${API})\n`);
    while (!["done", "error", "cancelled"].includes(job.status)) {
      await new Promise((r) => setTimeout(r, 1000));
      job = await (await fetch(`${API}/api/jobs/${job.id}`, { headers })).json();
      progress(job);
    }
  } else {
    const jobs = await import("../src/jobs.mjs");
    job = await local(jobs);
    process.stderr.write(`running ${job.id} on ${job.modelLabel}\n`);
    jobs.events.on("job", (j) => j.id === job.id && progress(j));
    job = await jobs.waitFor(job);
    await jobs.flush();
  }
  process.stderr.write("\n");
  if (job.status !== "done") throw new Error(job.error || job.status);
  const files = job.outputs.map((o) => path.join(OUTPUTS, o.file));
  if (out) {
    await fs.mkdir(out, { recursive: true });
    for (let i = 0; i < files.length; i++) { const d = path.join(out, path.basename(files[i])); await fs.copyFile(files[i], d); files[i] = d; }
  }
  console.log(files.join("\n"));
  process.exit(0);
}

let lastLine = "";
function progress(j) {
  const p = j.progress;
  const line = !p ? j.status : p.max ? `${p.stage} ${p.value}/${p.max}` : p.stage || j.status;
  if (line !== lastLine) { process.stderr.write(`\r\x1b[2K  ${line}`); lastLine = line; }
}

async function cmdDownload(pos, opt) {
  const [target, ...files] = pos;
  const { FAMILIES, downloadInfo } = await import("../src/families.mjs");
  if (!target) {
    console.log("Model families you can download:");
    for (const f of FAMILIES) {
      const d = downloadInfo(f);
      console.log(`  ${f.kind.padEnd(6)} ${f.id.padEnd(18)} ${gb(d.size).padStart(8)}  ${f.label}${d.gated ? "  (gated: accept the license at " + d.gated + ")" : ""}`);
      for (const r of d.repos) console.log(`  ${"".padEnd(36)}${r.url}`);
    }
    console.log("\nOr any Hugging Face repo: dogs download <org/repo> [file ...]   (find one with: dogs hf search <words>)");
    return;
  }
  const { startDownload } = await import("../src/downloads.mjs");
  const fam = FAMILIES.find((f) => f.id === target);
  let roles = null;
  if (fam && !opt.all && !files.length) {
    roles = await (await import("../src/hub.mjs")).missingRoles(target);
    if (roles) console.log(roles.length ? `Fetching only the missing ${roles.join(", ")} (use --all for everything)` : `${fam.label} is already complete.`);
    if (roles && !roles.length) return;
  }
  if (fam) {
    const d = downloadInfo(fam, roles);
    for (const it of d.items) console.log(`  ${gb(it.size).padStart(8)}  ${it.url}`);
    console.log(`  ${gb(d.size).padStart(8)}  total\n`);
  }
  const dl = startDownload(target, files, { roles, onLine: (l) => console.log(l) });
  const r = await dl.done;
  if (r.status !== "done") throw new Error(r.error);
  console.log(`\ndone. Check with: dogs models`);
}

async function cmdHf(pos, opt) {
  const [sub, ...args] = pos;
  const hub = await import("../src/hub.mjs");
  if (sub === "search" || sub === "ls" || sub === "list") {
    const results = await hub.searchModels({ query: args.join(" "), kind: opt.kind, limit: Number(opt.limit || 20), sort: opt.sort });
    if (opt.json) return console.log(JSON.stringify(results, null, 2));
    if (!results.length) return console.log("no matches");
    for (const m of results) {
      const tag = m.family ? `\x1b[32m[dogs: ${m.family}]\x1b[0m` : m.pipeline || "";
      console.log(`${m.repo.padEnd(52)} ${String(m.downloads).padStart(9)} dl  ${m.gated ? "\x1b[33mgated\x1b[0m " : ""}${tag}\n    ${m.url}`);
    }
    return console.log("\nFiles and sizes: dogs hf files <org/repo>    Download: dogs download <org/repo> <file ...>");
  }
  if (sub === "files") {
    if (!args[0]) throw new Error("usage: dogs hf files <org/repo>");
    const r = await hub.repoFiles(args[0]);
    if (opt.json) return console.log(JSON.stringify(r, null, 2));
    console.log(`${r.repo}${r.gated ? "  (gated)" : ""}  ${r.url}`);
    for (const f of r.files) console.log(`  ${gb(f.size).padStart(8)}  ${f.file}`);
    return console.log(`  ${gb(r.size).padStart(8)}  all weight files\n\nDownload: dogs download ${r.repo} <file ...>`);
  }
  if (sub === "download") return cmdDownload(args, opt);
  // Anything else goes straight to the real CLI: dogs hf auth login, dogs hf cache ls, ...
  const p = spawn("hf", pos, { stdio: "inherit", shell: process.platform === "win32" });
  p.on("exit", (c) => process.exit(c ?? 0));
}

async function cmdComfy(pos) {
  const c = await import("../src/comfy.mjs");
  const { ensureExtraPaths } = await import("../src/models.mjs");
  const sub = pos[0] || "status";
  if (sub === "start") { await ensureExtraPaths(); await c.startComfy(); console.log("ComfyUI is up"); }
  else if (sub === "stop") console.log((await c.stopComfy()) ? "stopped" : "not running (or not started by dogs)");
  else if (sub === "restart") { await c.stopComfy(); await ensureExtraPaths(); await c.startComfy(); console.log("ComfyUI restarted"); }
  else if (sub === "status") console.log((await c.comfyUp()) ? JSON.stringify(await c.comfyStats(), null, 2) : "ComfyUI is not running");
  else if (sub === "add-node") (await import("../src/downloads.mjs")).addNode(pos[1]);
  else throw new Error(`unknown: dogs comfy ${sub}`);
}

async function main() {
  const { pos, opt } = flags(rest);
  switch (cmd) {
    case "serve": {
      const { serve } = await import("../src/api.mjs");
      const s = await serve({ host: opt.host, port: opt.port ? Number(opt.port) : undefined });
      const a = s.address();
      console.log(`dogs API on http://${a.address}:${a.port}  (GET /api for endpoints)${API_KEY ? "  [API key required]" : ""}`);
      return;
    }
    case "models":
      if (pos[0] === "import") {
        const { getModel, prepareModel } = await import("../src/models.mjs");
        const m = await getModel(pos[1]);
        if (!m) throw new Error(`no model ${pos[1]} (see dogs models)`);
        await prepareModel(m, { copy: true });
        return console.log(`${m.label} is ready for ComfyUI`);
      }
      return cmdModels(opt);
    case "where": {
      const { listModels } = await import("../src/models.mjs");
      return printWhere(await listModels({ refresh: true }));
    }
    case "download": return cmdDownload(pos, opt);
    case "hf": return cmdHf(pos, opt);
    case "llm": {
      const r = await (await import("../src/llm.mjs")).listLlms();
      if (opt.json) return console.log(JSON.stringify(r, null, 2));
      if (!r.up) return console.log(`Ollama is not reachable at ${r.url}: ${r.error}`);
      const k = (n) => (n == null ? "?" : n >= 1024 ? `${Math.round(n / 1024)}k` : String(n));
      for (const m of r.models) {
        console.log(`${m.name.padEnd(28)} ${gb(m.size).padStart(8)}  ${(m.parameters || "").padEnd(6)} ${(m.quantization || "").padEnd(7)} ` +
          `ctx ${k(m.context.configured)} of ${k(m.context.max)}${m.loaded ? `  \x1b[32mloaded\x1b[0m (${gb(m.loaded.vram)} VRAM, ctx ${k(m.context.loaded)})` : ""}`);
      }
      return console.log(`\nOllama ${r.version}, ${gb(r.disk.bytes)} on disk in ${r.disk.dir} (aliases share weights)`);
    }
    case "gen": case "generate": return cmdGen(pos, opt);
    case "animate": return cmdAnimate(pos, opt);
    case "jobs": {
      const { listJobs } = await import("../src/jobs.mjs");
      for (const j of await listJobs({ limit: Number(opt.limit || 20) })) {
        console.log(`${new Date(j.created).toLocaleString()}  ${j.status.padEnd(9)} ${j.kind.padEnd(6)} ${j.prompt.slice(0, 60)}`);
        for (const o of j.outputs) console.log(`    ${path.join(OUTPUTS, o.file)}`);
      }
      return;
    }
    // ComfyUI inherits our stdio handles, which would keep this process alive: exit explicitly.
    case "comfy": await cmdComfy(pos); return process.exit(0);
    case "app": {
      const p = spawn(process.platform === "win32" ? "npm.cmd" : "npm", ["run", "desktop"], { cwd: path.join(REPO, "app"), stdio: "inherit", shell: process.platform === "win32" });
      return p.on("exit", (c) => process.exit(c ?? 0));
    }
    case undefined: case "help": case "--help": case "-h": return console.log(HELP);
    default: console.error(`unknown command "${cmd}"\n`); console.log(HELP); process.exit(2);
  }
}

main().catch((e) => { console.error(`\x1b[31merror:\x1b[0m ${e.message || e}`); process.exit(1); });
