// Model downloads through the Hugging Face CLI into the normal hub cache (~/.cache/huggingface/hub), where the
// scanner picks them up. Also installs the few ComfyUI custom nodes some model formats need.
import { spawn, spawnSync } from "node:child_process";
import { existsSync, promises as fs } from "node:fs";
import os from "node:os";
import path from "node:path";
import { randomUUID } from "node:crypto";
import { EventEmitter } from "node:events";
import { FAMILIES } from "./families.mjs";
import { COMFY_DIR, IS_WIN, OLLAMA_URL } from "./config.mjs";
import { ensureOllama } from "./llm.mjs";

export const events = new EventEmitter();
const REPO_RE = /^[A-Za-z0-9][\w.-]*\/[A-Za-z0-9][\w.-]*$/;
const FILE_RE = /^[\w][\w.\/-]*$/;
const downloads = new Map();

// On Windows `hf` is usually an hf.cmd shim, which only runs through a shell.
function hfCommand() {
  const probe = spawnSync("hf", ["version"], { stdio: "ignore", shell: IS_WIN });
  if (probe.status === 0) return ["hf"];
  return ["uvx", "--from", "huggingface_hub", "hf"];
}

/** Resolve a download target: a family id, or a raw repo with optional file list. */
export function planDownload(target, files = [], roles = null) {
  const fam = FAMILIES.find((f) => f.id === target);
  if (fam) {
    const byRepo = new Map();
    for (const it of fam.download.items) {
      if (roles && !roles.includes(it.role)) continue;
      byRepo.set(it.repo, [...(byRepo.get(it.repo) || []), it.file]);
    }
    return { label: fam.label, gated: fam.download.gated, steps: [...byRepo].map(([repo, fl]) => ({ repo, files: fl })) };
  }
  if (REPO_RE.test(target)) {
    // On Windows `hf` runs through a shell, so only plain repo-relative paths may reach the command line.
    if (!Array.isArray(files)) throw Object.assign(new Error("files must be an array of paths"), { status: 400 });
    const bad = files.find((f) => typeof f !== "string" || !FILE_RE.test(f) || f.split("/").includes(".."));
    if (bad !== undefined) throw Object.assign(new Error(`invalid file path "${bad}"`), { status: 400 });
    return { label: target, steps: [{ repo: target, files }] };
  }
  const e = new Error(`"${target}" is neither a model family (${FAMILIES.map((f) => f.id).join(", ")}) nor an org/repo id`);
  e.status = 400;
  throw e;
}

const PULL_LOG_MS = 1000;

// Chat templates for GGUFs that ship without one. Ollama then falls back to a bare "{{ .Prompt }}": roles are lost
// and nothing ends a reply, so the model rambles until the context fills. Recognized by tokenizer token ids.
const TEMPLATES = [
  {
    name: "Llama 3",
    match: (info) => info["tokenizer.ggml.bos_token_id"] === 128000,
    template: "{{- range .Messages }}<|start_header_id|>{{ .Role }}<|end_header_id|>\n\n{{ .Content }}<|eot_id|>{{ end }}<|start_header_id|>assistant<|end_header_id|>\n\n",
    stop: ["<|eot_id|>", "<|start_header_id|>", "<|end_header_id|>"],
  },
];

/** Give a just-installed model a chat template if its GGUF had none. */
export async function ensureTemplate(name, push = console.log) {
  const post = (p, body) => fetch(OLLAMA_URL + p, { method: "POST", headers: { "content-type": "application/json" }, body: JSON.stringify(body) });
  const show = await (await post("/api/show", { model: name })).json().catch(() => ({}));
  if (show.template && show.template.trim() !== "{{ .Prompt }}") return;
  const known = TEMPLATES.find((t) => t.match(show.model_info || {}));
  if (!known) { push(`warning: ${name} has no chat template; replies may not stop on their own`); return; }
  push(`no chat template in the file: using the ${known.name} template`);
  const res = await post("/api/create", { model: name, from: name, template: known.template, parameters: { stop: known.stop }, stream: false });
  if (!res.ok) push(`warning: could not set the chat template: ${await res.text()}`);
}

/** Run a command, feeding each output line to push. Resolves with the exit code (-1 if it could not start). */
function run(cmd, args, push, opts = {}) {
  return new Promise((resolve) => {
    const p = spawn(cmd, args, { windowsHide: true, ...opts });
    const feed = (d) => String(d).split(/\n|\r(?!\n)/).forEach(push);
    p.stdout.on("data", feed);
    p.stderr.on("data", feed);
    p.on("error", (e) => { push(`failed to run ${cmd}: ${e.message}`); resolve(-1); });
    p.on("exit", resolve);
  });
}

/**
 * Put a chat GGUF from Hugging Face into Ollama, tracked like any download. direct: Ollama pulls hf.co/org/repo:TAG
 * itself. Otherwise (repo names too long for Ollama) the file is downloaded to a temp folder, imported with
 * `ollama create` under the short name, and the temp copy removed.
 */
export function startOllamaInstall({ repo, file, name, direct }) {
  const dl = { id: randomUUID().slice(0, 8), target: name, label: name.replace(/^hf\.co\//, ""), status: "running", log: [], started: Date.now(), finished: null, error: null, ollama: true };
  downloads.set(dl.id, dl);
  const push = (line) => {
    line = line.replace(/\r/g, "").replace(/\x1b\[[\d;?]*[A-Za-z]/g, "").trimEnd();
    if (!line) return;
    dl.log.push(line);
    if (dl.log.length > 200) dl.log.splice(0, dl.log.length - 200);
    events.emit("download", dl);
  };
  const fail = (msg) => { Object.assign(dl, { status: "error", error: msg, finished: Date.now() }); events.emit("download", dl); return dl; };
  const finish = () => { Object.assign(dl, { status: "done", finished: Date.now() }); events.emit("download", dl); return dl; };
  dl.done = (async () => {
    await ensureOllama();
    if (!direct) {
      const dir = path.join(os.tmpdir(), `dogs-ollama-${dl.id}`);
      try {
        const hf = hfCommand();
        push(`> hf download ${repo} ${file}`);
        const code = await run(hf[0], [...hf.slice(1), "download", repo, file, "--local-dir", dir], push, { shell: IS_WIN && hf[0] === "hf", env: { ...process.env, HF_HUB_DISABLE_SYMLINKS_WARNING: "1" } });
        if (code !== 0) return fail(`hf download ${repo} failed (exit ${code})`);
        await fs.writeFile(path.join(dir, "Modelfile"), `FROM ./${file}\n`);
        push(`> ollama create ${name}`);
        const made = await run("ollama", ["create", name, "-f", "Modelfile"], push, { cwd: dir });
        if (made !== 0) return fail(`ollama create ${name} failed (exit ${made})`);
        await ensureTemplate(name, push);
        return finish();
      } finally {
        await fs.rm(dir, { recursive: true, force: true }).catch(() => {});
      }
    }
    const model = name;
    push(`> ollama pull ${model}`);
    let res;
    // ensureOllama may have only just started `ollama serve`: give it a few seconds to answer.
    for (let i = 0; !res; i++) {
      try {
        res = await fetch(OLLAMA_URL + "/api/pull", { method: "POST", headers: { "content-type": "application/json" }, body: JSON.stringify({ model, stream: true }) });
      } catch (e) {
        if (i >= 10) return fail(`Could not reach Ollama at ${OLLAMA_URL}: ${e.message}. Is it installed and running?`);
        await new Promise((r) => setTimeout(r, 1000));
      }
    }
    if (!res.ok || !res.body) return fail(`Ollama pull -> HTTP ${res.status}: ${await res.text().catch(() => "")}`);
    let buf = "", last = 0;
    const dec = new TextDecoder();
    for await (const chunk of res.body) {
      buf += dec.decode(chunk, { stream: true });
      let nl;
      while ((nl = buf.indexOf("\n")) >= 0) {
        const line = buf.slice(0, nl).trim();
        buf = buf.slice(nl + 1);
        if (!line) continue;
        let msg;
        try { msg = JSON.parse(line); } catch { continue; }
        if (msg.error) return fail(`Ollama: ${msg.error}`);
        // Byte progress arrives many times a second: log it at most once a second.
        if (msg.total && msg.completed != null) {
          if (Date.now() - last < PULL_LOG_MS && msg.completed < msg.total) continue;
          last = Date.now();
          push(`${msg.status} ${Math.floor((msg.completed / msg.total) * 100)}% (${(msg.completed / 1e9).toFixed(2)} / ${(msg.total / 1e9).toFixed(2)} GB)`);
        } else if (msg.status) push(msg.status);
      }
    }
    if (dl.log.at(-1) !== "success") return fail("Ollama stopped before the pull finished");
    await ensureTemplate(model, push);
    return finish();
  })();
  return dl;
}

export function listDownloads() { return [...downloads.values()].reverse(); }
export function getDownload(id) { return downloads.get(id) || null; }

/** Start a download in the background. onLine gets every output line (used by the CLI to stream). */
export function startDownload(target, files = [], { onLine, roles = null } = {}) {
  const plan = planDownload(target, files, roles);
  if (!plan.steps.length) throw Object.assign(new Error(`${plan.label}: nothing to download, every file is already installed`), { status: 400 });
  const dl = { id: randomUUID().slice(0, 8), target, label: plan.label, status: "running", log: [], started: Date.now(), finished: null, error: null };
  downloads.set(dl.id, dl);
  const push = (line) => {
    line = line.replace(/\r/g, "").trimEnd();
    if (!line) return;
    dl.log.push(line);
    if (dl.log.length > 200) dl.log.splice(0, dl.log.length - 200);
    onLine?.(line);
    events.emit("download", dl);
  };
  const hf = hfCommand();
  dl.done = (async () => {
    for (const step of plan.steps) {
      push(`> hf download ${step.repo} ${step.files.join(" ")}`);
      const code = await new Promise((resolve) => {
        const p = spawn(hf[0], [...hf.slice(1), "download", step.repo, ...step.files], { windowsHide: true, shell: IS_WIN && hf[0] === "hf", env: { ...process.env, HF_HUB_DISABLE_SYMLINKS_WARNING: "1" } });
        const feed = (d) => String(d).split(/\n|\r(?!\n)/).forEach(push);
        p.stdout.on("data", feed);
        p.stderr.on("data", feed);
        p.on("error", (e) => { push(`failed to run ${hf[0]}: ${e.message}`); resolve(-1); });
        p.on("exit", resolve);
      });
      if (code !== 0) {
        const gatedHint = dl.log.some((l) => /401|403|gated|restricted|access/i.test(l))
          ? ` This repo is gated or needs a login: accept the license at https://huggingface.co/${step.repo} and run \`hf auth login\` (or set HF_TOKEN).`
          : "";
        Object.assign(dl, { status: "error", error: `hf download ${step.repo} failed (exit ${code}).${gatedHint}`, finished: Date.now() });
        events.emit("download", dl);
        return dl;
      }
    }
    Object.assign(dl, { status: "done", finished: Date.now() });
    events.emit("download", dl);
    return dl;
  })();
  return dl;
}

export const CUSTOM_NODES = {
  gguf: { repo: "https://github.com/city96/ComfyUI-GGUF.git", dir: "ComfyUI-GGUF", pip: ["gguf"], why: "load .gguf quantized diffusion models" },
};

/** git clone a custom node into ComfyUI and install its Python deps into ComfyUI's venv. */
export function addNode(name, onLine = console.log) {
  const n = CUSTOM_NODES[name];
  if (!n) throw new Error(`unknown node pack "${name}" (known: ${Object.keys(CUSTOM_NODES).join(", ")})`);
  const dest = path.join(COMFY_DIR, "custom_nodes", n.dir);
  const py = path.join(COMFY_DIR, ".venv", IS_WIN ? "Scripts/python.exe" : "bin/python");
  const run = (cmd, args) => {
    onLine(`> ${cmd} ${args.join(" ")}`);
    const r = spawnSync(cmd, args, { stdio: "inherit" });
    if (r.status !== 0) throw new Error(`${cmd} failed (exit ${r.status})`);
  };
  if (existsSync(dest)) run("git", ["-C", dest, "pull", "--ff-only"]);
  else run("git", ["clone", "--depth", "1", n.repo, dest]);
  if (n.pip.length) run("uv", ["pip", "install", "--python", py, ...n.pip]);
  onLine(`installed ${n.dir}. Restart ComfyUI (dogs comfy restart) to load it.`);
}
