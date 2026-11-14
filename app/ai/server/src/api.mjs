// HTTP API used by the desktop app, the CLI, and outside callers (e.g. an assistant that needs an image).
// No dependencies: node:http, JSON in and out, Server-Sent Events for live updates.
import http from "node:http";
import { createReadStream, promises as fs } from "node:fs";
import path from "node:path";
import { timingSafeEqual } from "node:crypto";
import { API_HOST, API_PORT, API_KEY, OUTPUTS, REPO, COMFY_URL } from "./config.mjs";
import { listModels, deleteFiles, ensureExtraPaths } from "./models.mjs";
import { submit, animate, listJobs, getJob, cancel, removeJob, waitFor, activeJobs, events as jobEvents, mimeOf } from "./jobs.mjs";
import { clearDatabase } from "./db.mjs";
import { startDownload, startOllamaInstall, listDownloads, getDownload, events as dlEvents } from "./downloads.mjs";
import { comfyUp, comfyStats, startComfy, stopComfy, freeComfy, comfyStarting, startError } from "./comfy.mjs";
import { searchModels, repoFiles, missingRoles, ollamaName } from "./hub.mjs";
import { listLlms, unloadLlm, deleteLlm, ensureOllama } from "./llm.mjs";
import * as chat from "./chat.mjs";
import * as characters from "./characters.mjs";
import * as prompts from "./prompts.mjs";
import * as storage from "./storage.mjs";

const APP_DIST = path.join(REPO, "app", "dist");
const STATIC_MIME = { ".html": "text/html; charset=utf-8", ".js": "text/javascript", ".css": "text/css", ".svg": "image/svg+xml", ".png": "image/png", ".ico": "image/x-icon", ".json": "application/json", ".woff2": "font/woff2" };

const ENDPOINTS = [
  ["GET", "/api/health", "ComfyUI status, GPU and queue"],
  ["GET", "/api/models?kind=image|video|audio&refresh=1", "Installed models (ready / incomplete / broken), scanned locations, unrecognized files"],
  ["POST", "/api/generate", "{ kind | model, prompt, ...params, seed?, wait? } -> job. With wait:true the response is the finished job"],
  ["POST", "/api/animate", "{ image: outputs path | data: URL, motion?, model?, seconds?, seed?, wait? } -> video job starting from that image"],
  ["GET", "/api/jobs?limit=&kind=&q=&status=", "Jobs newest first (q searches prompts), from the SQLite history"],
  ["GET", "/api/jobs/:id", "One job; outputs[].url is downloadable"],
  ["DELETE", "/api/jobs/:id?files=1", "Cancel a queued/running job, or delete a finished one from history (files=1 also deletes its files)"],
  ["GET", "/files/<path>", "Generated files"],
  ["GET", "/api/storage?refresh=1", "Every folder this stack uses on disk, with sizes (hard links counted once)"],
  ["POST", "/api/storage/:id/open", "Open that folder in Explorer / Finder"],
  ["POST", "/api/storage/:id/clear", "Empty a cache folder (only cache places can be cleared)"],
  ["POST", "/api/database/clear", "Empty the history database: jobs, your prompts, chats, characters (built-in prompts and files in outputs/ stay). Refused while jobs run"],
  ["GET", "/api/characters", "Characters (with media counts)"],
  ["POST", "/api/characters", "{ name?, bio?, lists? } -> new character"],
  ["GET", "/api/characters/:id", "A character with its media"],
  ["POST", "/api/characters/:id", "{ name?, bio?, lists?: [{ title, items }], avatar?: mediaId } -> update"],
  ["POST", "/api/characters/:id/media", "Raw upload (application/octet-stream + x-filename) or JSON { file } from outputs/"],
  ["DELETE", "/api/characters/:id/media/:mediaId", "Remove a media item (uploaded files are deleted)"],
  ["DELETE", "/api/characters/:id", "Delete a character"],
  ["GET", "/api/prompts?q=&category=", "Saved prompts, most recently used first (every generation records its prompt)"],
  ["POST", "/api/prompts", "{ text, title?, category?, kind?, notes? } -> save a prompt (existing text updates that entry)"],
  ["POST", "/api/prompts/:id", "{ title?, category?, notes? } -> update"],
  ["DELETE", "/api/prompts/:id", "Delete a prompt"],
  ["GET", "/api/chats", "Chat conversations (newest first)"],
  ["POST", "/api/chats", "{ model?, characterId? } -> new conversation (optionally as a character)"],
  ["GET", "/api/chats/:id", "A conversation with its messages"],
  ["POST", "/api/chats/:id/messages", "{ content, model?, think? } -> streams the reply as NDJSON: thinking/token events, then done"],
  ["DELETE", "/api/chats/:id", "Delete a conversation"],
  ["GET", "/api/llm", "Ollama LLMs: size, context window (max / configured / loaded), what is in memory, disk use"],
  ["POST", "/api/llm/:name/unload", "Unload an LLM from memory"],
  ["DELETE", "/api/llm/:name", "Delete an Ollama model"],
  ["POST", "/api/models/delete", "{ ids: [file id] } -> delete scanned weight files (plus their cache links)"],
  ["GET", "/api/hub/search?q=&kind=all|image|video|audio|chat&sort=downloads|likes|trending|modified|created&format=gguf|safetensors|diffusers&author=&limit=", "Search Hugging Face models (q may be a pasted link or org/name)"],
  ["POST", "/api/llm/pull", "{ repo: org/name, file: x.gguf } -> add a chat GGUF from Hugging Face to Ollama"],
  ["GET", "/api/hub/files?repo=org/name", "Weight files in a Hugging Face repo, with sizes and links"],
  ["POST", "/api/downloads", "{ target: <family id or org/repo>, files?: [], all?: bool } -> start a Hugging Face download (families fetch only missing files)"],
  ["GET", "/api/downloads", "Download progress"],
  ["POST", "/api/comfy/start | /api/comfy/stop | /api/comfy/free", "Control the ComfyUI process / VRAM"],
  ["GET", "/api/events", "Server-Sent Events: job and download updates"],
];

function send(res, status, body, headers = {}) {
  const data = typeof body === "string" ? body : JSON.stringify(body, null, 1);
  res.writeHead(status, { "content-type": typeof body === "string" ? "text/plain; charset=utf-8" : "application/json", ...headers });
  res.end(data);
}

async function readBody(req) {
  const chunks = [];
  if (!String(req.headers["content-type"] || "").startsWith("application/json") && req.headers["content-length"] !== "0") {
    if (req.method === "POST") throw Object.assign(new Error("expected a JSON body"), { status: 415 });
  }
  let size = 0;
  for await (const c of req) { size += c.length; if (size > 25 * 2 ** 20) throw Object.assign(new Error("body too large"), { status: 413 }); chunks.push(c); }
  if (!chunks.length) return {};
  try { return JSON.parse(Buffer.concat(chunks).toString("utf8")); } catch { throw Object.assign(new Error("body must be JSON"), { status: 400 }); }
}

const sameKey = (given) => {
  const a = Buffer.from(String(given || "")), b = Buffer.from(API_KEY);
  return a.length === b.length && timingSafeEqual(a, b);
};

function authorized(req, url) {
  if (!API_KEY) return true;
  const h = req.headers.authorization || "";
  if (sameKey(h.startsWith("Bearer ") ? h.slice(7) : "")) return true;
  // EventSource and <img>/<video> cannot set headers, so ?key= is accepted for those reads only (it can end
  // up in logs and history, so never for anything that changes state).
  const keyInUrlOk = req.method === "GET" && (url.pathname.startsWith("/files/") || url.pathname === "/api/events");
  return keyInUrlOk && sameKey(url.searchParams.get("key"));
}

// Browsers may only call the API from the app itself (served here, the Tauri shell, the Vite dev server)
// or origins listed in DOGS_ORIGINS. Without this, any website open on this machine could drive the local
// engine. Requests with no Origin (curl, the CLI, server-to-server callers) are unaffected.
const ALLOWED_ORIGINS = new Set([
  "tauri://localhost", "http://tauri.localhost", "https://tauri.localhost",
  "http://localhost:5173", "http://127.0.0.1:5173",
  ...(process.env.DOGS_ORIGINS || "").split(",").map((s) => s.trim()).filter(Boolean),
]);
const LOCAL_HOSTS = new Set(["127.0.0.1", "localhost", "[::1]"]);

function originAllowed(req) {
  const origin = req.headers.origin;
  if (!origin) return true;
  return ALLOWED_ORIGINS.has(origin) || origin === `http://${req.headers.host}` || origin === `https://${req.headers.host}`;
}

// Without a key the API trusts localhost only, so the Host header must be local too: that stops DNS-rebinding
// pages (evil.example resolving to 127.0.0.1) from posing as same-origin.
function hostAllowed(req) {
  if (API_KEY) return true;
  const host = String(req.headers.host || "").replace(/:\d+$/, "").toLowerCase();
  return LOCAL_HOSTS.has(host);
}

async function serveFile(req, res, file, mime) {
  let st;
  try { st = await fs.stat(file); } catch { return send(res, 404, { error: "not found" }); }
  if (!st.isFile()) return send(res, 404, { error: "not found" });
  const range = /bytes=(\d*)-(\d*)/.exec(req.headers.range || "");
  const base = { "content-type": mime, "accept-ranges": "bytes", "cache-control": "no-cache" };
  if (range) {
    const start = range[1] ? Number(range[1]) : 0;
    const end = range[2] ? Math.min(Number(range[2]), st.size - 1) : st.size - 1;
    res.writeHead(206, { ...base, "content-range": `bytes ${start}-${end}/${st.size}`, "content-length": end - start + 1 });
    createReadStream(file, { start, end }).pipe(res);
  } else {
    res.writeHead(200, { ...base, "content-length": st.size });
    createReadStream(file).pipe(res);
  }
}

// Resolve a request path inside a root, refusing anything that escapes it.
function inside(root, rel) {
  const p = path.resolve(root, "." + path.sep + decodeURIComponent(rel));
  return p === root || p.startsWith(root + path.sep) ? p : null;
}

function sse(req, res) {
  res.writeHead(200, { "content-type": "text/event-stream", "cache-control": "no-cache", connection: "keep-alive" });
  res.write(": connected\n\n");
  const onJob = (j) => res.write(`event: job\ndata: ${JSON.stringify(j)}\n\n`);
  const onDl = (d) => res.write(`event: download\ndata: ${JSON.stringify({ ...d, done: undefined })}\n\n`);
  jobEvents.on("job", onJob);
  dlEvents.on("download", onDl);
  const ping = setInterval(() => res.write(": ping\n\n"), 25_000);
  req.on("close", () => { clearInterval(ping); jobEvents.off("job", onJob); dlEvents.off("download", onDl); });
}

async function route(req, res, url) {
  const p = url.pathname;
  const m = req.method;

  if (p === "/api" || p === "/api/") return send(res, 200, { name: "dogs-ai", endpoints: ENDPOINTS.map(([method, route, about]) => ({ method, route, about })) });
  if (p === "/api/health" && m === "GET") {
    const up = await comfyUp();
    const state = up ? await comfyStats().catch((e) => ({ error: e.message })) : { starting: comfyStarting(), error: startError || undefined };
    return send(res, 200, { ok: true, comfy: { url: COMFY_URL, up, ...state }, auth: !!API_KEY });
  }
  if (p === "/api/models" && m === "GET") {
    const r = await listModels({ refresh: url.searchParams.has("refresh") });
    const kind = url.searchParams.get("kind");
    return send(res, 200, kind ? { ...r, models: r.models.filter((x) => x.kind === kind), families: r.families.filter((f) => f.kind === kind) } : r);
  }
  if (p === "/api/generate" && m === "POST") {
    const body = await readBody(req);
    const job = await submit(body);
    if (body.wait) return send(res, 200, await waitFor(job));
    return send(res, 202, job);
  }
  if (p === "/api/animate" && m === "POST") {
    const body = await readBody(req);
    const job = await animate(body);
    if (body.wait) return send(res, 200, await waitFor(job));
    return send(res, 202, job);
  }
  if (p === "/api/jobs" && m === "GET") {
    const s = url.searchParams;
    return send(res, 200, await listJobs({ limit: Math.min(1000, Number(s.get("limit") || 100)), kind: s.get("kind") || undefined, q: s.get("q") || undefined, status: s.get("status") || undefined }));
  }
  const jm = /^\/api\/jobs\/([\w-]+)$/.exec(p);
  if (jm && m === "GET") { const j = await getJob(jm[1]); return j ? send(res, 200, j) : send(res, 404, { error: "no such job" }); }
  if (jm && m === "DELETE") {
    // In flight: cancel it. Finished: remove it from history (and its files with ?files=1).
    const j = await getJob(jm[1]);
    if (!j) return send(res, 404, { error: "no such job" });
    if (["queued", "running"].includes(j.status)) return send(res, 200, await cancel(j.id));
    return send(res, 200, { deleted: (await removeJob(j.id, { files: url.searchParams.get("files") === "1" })).id });
  }

  if (p === "/api/downloads" && m === "GET") return send(res, 200, listDownloads().map((d) => ({ ...d, done: undefined })));
  if (p === "/api/downloads" && m === "POST") {
    const { target, files = [], all = false } = await readBody(req);
    // For a family, fetch only what the installed copy is missing unless all:true.
    const roles = all || files.length ? null : await missingRoles(String(target || ""));
    const dl = startDownload(String(target || ""), files, { roles });
    return send(res, 202, { ...dl, done: undefined });
  }
  if (p === "/api/storage" && m === "GET") return send(res, 200, await storage.storageReport({ refresh: url.searchParams.has("refresh") }));
  const sm = /^\/api\/storage\/([\w-]+)\/(open|clear)$/.exec(p);
  if (sm && m === "POST") return send(res, 200, sm[2] === "open" ? await storage.openPlace(sm[1]) : await storage.clearPlace(sm[1]));
  if (p === "/api/database/clear" && m === "POST") {
    if (activeJobs()) return send(res, 409, { error: "jobs are still queued or running; wait for them or cancel them first" });
    return send(res, 200, { removed: await clearDatabase() });
  }
  if (p === "/api/characters" && m === "GET") return send(res, 200, await characters.listCharacters());
  if (p === "/api/characters" && m === "POST") return send(res, 201, await characters.createCharacter(await readBody(req)));
  const chm = /^\/api\/characters\/([\w-]+)(?:\/media(?:\/([\w-]+))?)?$/.exec(p);
  if (chm) {
    const [, id, mediaId] = chm;
    const isMedia = p.includes("/media");
    if (!isMedia && m === "GET") { const c = await characters.getCharacter(id); return c ? send(res, 200, c) : send(res, 404, { error: "no such character" }); }
    if (!isMedia && m === "POST") return send(res, 200, await characters.updateCharacter(id, await readBody(req)));
    if (!isMedia && m === "DELETE") { const c = await characters.deleteCharacter(id); return c ? send(res, 200, { deleted: id }) : send(res, 404, { error: "no such character" }); }
    if (isMedia && !mediaId && m === "POST") {
      // Raw upload (x-filename header), or JSON { file } to attach something already in outputs/.
      if (String(req.headers["content-type"]).startsWith("application/octet-stream")) {
        return send(res, 201, await characters.attachUpload(id, req, req.headers["x-filename"]));
      }
      return send(res, 201, await characters.attachExisting(id, (await readBody(req)).file));
    }
    if (isMedia && mediaId && m === "DELETE") return send(res, 200, await characters.removeMedia(id, mediaId));
  }
  if (p === "/api/prompts" && m === "GET") {
    return send(res, 200, await prompts.listPrompts({ q: url.searchParams.get("q") || undefined, category: url.searchParams.get("category") || undefined }));
  }
  if (p === "/api/prompts" && m === "POST") return send(res, 201, await prompts.createPrompt(await readBody(req)));
  const pm = /^\/api\/prompts\/([\w-]+)$/.exec(p);
  if (pm && m === "POST") return send(res, 200, await prompts.updatePrompt(pm[1], await readBody(req)));
  if (pm && m === "DELETE") { const d = await prompts.deletePrompt(pm[1]); return d ? send(res, 200, { deleted: pm[1] }) : send(res, 404, { error: "no such prompt" }); }
  if (p === "/api/chats" && m === "GET") return send(res, 200, await chat.listChats());
  if (p === "/api/chats" && m === "POST") return send(res, 201, await chat.newChat(await readBody(req)));
  const cm = /^\/api\/chats\/([\w-]+)(\/messages)?$/.exec(p);
  if (cm && !cm[2] && m === "GET") { const c = await chat.getChat(cm[1]); return c ? send(res, 200, c) : send(res, 404, { error: "no such chat" }); }
  if (cm && !cm[2] && m === "DELETE") { await chat.deleteChat(cm[1]); return send(res, 200, { deleted: cm[1] }); }
  if (cm && cm[2] && m === "POST") {
    // Streams the reply as newline-delimited JSON. Closing the connection (Stop) aborts generation.
    const body = await readBody(req);
    const abort = new AbortController();
    res.on("close", () => { if (!res.writableEnded) abort.abort(); });
    const write = (ev) => { if (!res.writableEnded) res.write(JSON.stringify(ev) + "\n"); };
    try {
      res.writeHead(200, { "content-type": "application/x-ndjson", "cache-control": "no-cache" });
      await chat.sendMessage(cm[1], body, write, abort.signal);
    } catch (e) {
      write({ type: "error", error: String(e.message || e) });
    }
    return res.end();
  }
  if (p === "/api/models/delete" && m === "POST") {
    const { ids } = await readBody(req);
    if (!Array.isArray(ids) || !ids.length) return send(res, 400, { error: "ids: [file id, ...] required" });
    return send(res, 200, await deleteFiles(ids.map(String)));
  }
  if (p === "/api/llm" && m === "GET") return send(res, 200, await listLlms());
  if (p === "/api/llm/pull" && m === "POST") {
    const { repo = "", file = "" } = await readBody(req);
    if (!/^[\w.-]+\/[\w.-]+$/.test(repo)) return send(res, 400, { error: "repo must look like org/name" });
    const { files } = await repoFiles(repo);
    if (!files.some((f) => f.file === file)) return send(res, 404, { error: `${file} is not a weight file in ${repo}` });
    const target = ollamaName(repo, file, files);
    if (!target) return send(res, 400, { error: `${file} can't be run by Ollama on its own (split or vision-projector GGUF)` });
    const dl = startOllamaInstall({ repo, file, ...target });
    return send(res, 202, { ...dl, done: undefined });
  }
  const dlm = /^\/api\/llm\/(.+)$/.exec(p);
  if (dlm && m === "DELETE") { await deleteLlm(decodeURIComponent(dlm[1])); return send(res, 200, { deleted: true }); }
  const um = /^\/api\/llm\/(.+)\/unload$/.exec(p);
  if (um && m === "POST") { await unloadLlm(decodeURIComponent(um[1])); return send(res, 200, { unloaded: true }); }
  if (p === "/api/hub/search" && m === "GET") {
    const s = url.searchParams;
    return send(res, 200, await searchModels({
      query: s.get("q") || "", kind: s.get("kind") || undefined, limit: Math.min(100, Number(s.get("limit") || 20)),
      sort: s.get("sort") || undefined, format: s.get("format") || undefined, author: s.get("author") || undefined,
    }));
  }
  if (p === "/api/hub/files" && m === "GET") {
    const repo = url.searchParams.get("repo") || "";
    if (!/^[\w.-]+\/[\w.-]+$/.test(repo)) return send(res, 400, { error: "repo must look like org/name" });
    return send(res, 200, await repoFiles(repo));
  }
  const dm = /^\/api\/downloads\/([\w-]+)$/.exec(p);
  if (dm && m === "GET") { const d = getDownload(dm[1]); return d ? send(res, 200, { ...d, done: undefined }) : send(res, 404, { error: "no such download" }); }

  if (p === "/api/comfy/start" && m === "POST") { await startComfy(); return send(res, 200, { up: true }); }
  if (p === "/api/comfy/stop" && m === "POST") return send(res, 200, { stopped: await stopComfy() });
  if (p === "/api/comfy/free" && m === "POST") { await freeComfy(); return send(res, 200, { freed: true }); }
  if (p === "/api/events" && m === "GET") return sse(req, res);

  if (p.startsWith("/files/") && m === "GET") {
    const f = inside(OUTPUTS, p.slice("/files/".length));
    return f ? serveFile(req, res, f, mimeOf(f)) : send(res, 400, { error: "bad path" });
  }
  // The built desktop app is also served here, so any browser on the network can use it.
  if (m === "GET" && !p.startsWith("/api/")) {
    let f = inside(APP_DIST, p === "/" ? "index.html" : p.slice(1));
    if (f) {
      try { if (!(await fs.stat(f)).isFile()) throw 0; } catch { f = path.join(APP_DIST, "index.html"); }
      try { await fs.access(f); return serveFile(req, res, f, STATIC_MIME[path.extname(f)] || "application/octet-stream"); } catch {}
    }
    return send(res, 404, "dogs-ai API. The app is not built yet: run `npm run build` in app/. Endpoints: GET /api");
  }
  return send(res, 404, { error: `no route ${m} ${p}` });
}

export function serve({ host = API_HOST, port = API_PORT } = {}) {
  if (!API_KEY && !["127.0.0.1", "localhost", "::1"].includes(host)) {
    throw new Error(`refusing to listen on ${host} without DOGS_API_KEY: anyone on the network could use the GPU and delete files. ` +
      `Set DOGS_API_KEY to a long random string first.`);
  }
  const server = http.createServer(async (req, res) => {
    if (!hostAllowed(req)) return send(res, 403, { error: "unexpected Host header (set DOGS_API_KEY to serve other hosts)" });
    if (!originAllowed(req)) return send(res, 403, { error: `origin ${req.headers.origin} is not allowed (add it to DOGS_ORIGINS)` });
    if (req.headers.origin) {
      res.setHeader("access-control-allow-origin", req.headers.origin);
      res.setHeader("vary", "Origin");
      res.setHeader("access-control-allow-headers", "authorization, content-type");
      res.setHeader("access-control-allow-methods", "GET, POST, DELETE, OPTIONS");
    }
    if (req.method === "OPTIONS") return res.writeHead(204).end();
    // Writes must be JSON: browsers can't send that cross-site without a CORS preflight (which we refuse).
    // (Raw uploads use application/octet-stream, which also needs a preflight.)
    const ctype = String(req.headers["content-type"] || "");
    if (req.method === "POST" && !ctype.startsWith("application/json") && !ctype.startsWith("application/octet-stream")) {
      return send(res, 415, { error: "POST bodies must be content-type: application/json" });
    }
    const url = new URL(req.url, "http://x");
    const isApp = req.method === "GET" && !url.pathname.startsWith("/api") && !url.pathname.startsWith("/files/");
    if (!isApp && !authorized(req, url)) return send(res, 401, { error: "missing or wrong API key (Authorization: Bearer <DOGS_API_KEY>)" });
    try { await route(req, res, url); } catch (e) {
      if (!res.headersSent) send(res, e.status || 500, { error: String(e.message || e) });
      else res.end();
    }
  });
  return new Promise((resolve) => server.listen(port, host, () => {
    warmUp().catch((e) => console.error("[dogs]", e.message));
    resolve(server);
  }));
}

// Bring up what chat and generation need as soon as the engine starts, so the first request neither waits on
// them nor is the first to find out one is broken (/api/health reports a failed ComfyUI start).
async function warmUp() {
  await ensureOllama();
  await ensureExtraPaths();
  await startComfy();
}
