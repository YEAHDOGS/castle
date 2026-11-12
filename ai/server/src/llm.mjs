// Local LLMs served by Ollama: what is installed, how big, each model's context window (the model's maximum and
// the num_ctx actually configured), and what is loaded in memory right now.
import { spawn } from "node:child_process";
import { promises as fs } from "node:fs";
import os from "node:os";
import path from "node:path";
import { OLLAMA_URL } from "./config.mjs";

const TIMEOUT_MS = 4000;

async function ollama(pathname, body) {
  const init = body ? { method: "POST", headers: { "content-type": "application/json" }, body: JSON.stringify(body) } : {};
  const res = await fetch(OLLAMA_URL + pathname, { ...init, signal: AbortSignal.timeout(TIMEOUT_MS) });
  if (!res.ok) throw new Error(`Ollama ${pathname} -> HTTP ${res.status}`);
  return res.json();
}

/** Start `ollama serve` when nothing answers (the Ollama tray app usually runs it already). */
export async function ensureOllama() {
  try { await ollama("/api/version"); return; } catch {}
  spawn("ollama", ["serve"], { detached: true, windowsHide: true, stdio: "ignore" })
    .on("error", (e) => console.error("[dogs] could not start Ollama:", e.message)).unref();
}

/** num_ctx and friends from the Modelfile "parameters" text block. */
function parseParameters(text = "") {
  const out = {};
  for (const line of text.split("\n")) {
    const m = line.trim().match(/^(\S+)\s+(.+)$/);
    if (m) out[m[1]] = isNaN(Number(m[2])) ? m[2].replace(/^"|"$/g, "") : Number(m[2]);
  }
  return out;
}

// Real disk use: tags made from the same weights (aliases) share blobs, so sum the blob store instead of tag sizes.
async function blobStoreBytes() {
  const dir = path.join(process.env.OLLAMA_MODELS || path.join(os.homedir(), ".ollama", "models"), "blobs");
  let total = 0;
  try {
    for (const e of await fs.readdir(dir, { withFileTypes: true })) {
      if (e.isFile()) total += (await fs.stat(path.join(dir, e.name))).size;
    }
  } catch { return { dir, bytes: null }; }
  return { dir, bytes: total };
}

export async function listLlms() {
  let tags, ps, version;
  try {
    [tags, ps, version] = await Promise.all([ollama("/api/tags"), ollama("/api/ps"), ollama("/api/version")]);
  } catch (e) {
    return { up: false, url: OLLAMA_URL, error: String(e.message || e), models: [], disk: await blobStoreBytes() };
  }
  const loaded = new Map((ps.models || []).map((m) => [m.name, m]));
  const models = await Promise.all((tags.models || []).map(async (m) => {
    let show = {};
    try { show = await ollama("/api/show", { model: m.name }); } catch {}
    const info = show.model_info || {};
    const maxCtx = Object.entries(info).find(([k]) => k.endsWith(".context_length"))?.[1] ?? null;
    const params = parseParameters(show.parameters);
    const live = loaded.get(m.name);
    return {
      name: m.name, size: m.size, digest: m.digest, modified: m.modified_at,
      family: m.details?.family || null, parameters: m.details?.parameter_size || null, quantization: m.details?.quantization_level || null,
      context: { max: maxCtx, configured: params.num_ctx ?? null, loaded: live?.context_length ?? null },
      capabilities: show.capabilities || [],
      sampling: Object.fromEntries(Object.entries(params).filter(([k]) => k !== "num_ctx" && k !== "stop")),
      loaded: live ? { size: live.size, vram: live.size_vram, until: live.expires_at } : null,
    };
  }));
  // Tags that are aliases of the same weights: same family, quantization and (nearly) the same size.
  for (const m of models) {
    m.sharesWeightsWith = models
      .filter((o) => o !== m && o.family === m.family && o.quantization === m.quantization && Math.abs(o.size - m.size) < 1e6)
      .map((o) => o.name);
  }
  return { up: true, url: OLLAMA_URL, version: version.version, models, disk: await blobStoreBytes() };
}

/** Remove a model tag from Ollama (weights go once no other tag uses them). */
export async function deleteLlm(name) {
  const res = await fetch(OLLAMA_URL + "/api/delete", { method: "DELETE", headers: { "content-type": "application/json" }, body: JSON.stringify({ model: name }), signal: AbortSignal.timeout(TIMEOUT_MS) });
  if (!res.ok) throw new Error(`Ollama delete ${name} -> HTTP ${res.status}: ${await res.text()}`);
}

/** Unload a model from memory now (it reloads on the next chat). */
export async function unloadLlm(name) {
  await ollama("/api/generate", { model: name, keep_alive: 0 });
}
