// Hugging Face Hub browsing through its public REST API (the same one the `hf` CLI uses): search models by
// kind, list a repo's weight files with sizes, and work out which files to fetch for a family.
import { readFileSync } from "node:fs";
import os from "node:os";
import path from "node:path";
import { HF, FAMILIES } from "./families.mjs";
import { WEIGHT_EXT } from "./scan.mjs";
import { listModels } from "./models.mjs";

// Hub pipeline tags per kind of output.
const PIPELINES = {
  image: ["text-to-image", "image-to-image"],
  video: ["text-to-video", "image-to-video"],
  audio: ["text-to-audio", "text-to-speech"],
  chat: ["text-generation", "image-text-to-text"],
};
const SORTS = { downloads: "downloads", likes: "likes", trending: "trendingScore", modified: "lastModified", created: "createdAt" };
const FORMATS = ["gguf", "safetensors", "diffusers"];
// A repo id typed or pasted as a link: huggingface.co/org/name, hf.co/org/name/blob/main/..., or just org/name.
const REPO_LINK = /^(?:https?:\/\/)?(?:www\.)?(?:huggingface\.co|hf\.co)\/(?:models\/)?([\w.-]+\/[\w.-]+)/i;
const REPO_ID = /^([A-Za-z0-9][\w.-]*\/[A-Za-z0-9][\w.-]*)$/;

function token() {
  if (process.env.HF_TOKEN) return process.env.HF_TOKEN;
  const home = process.env.HF_HOME || path.join(os.homedir(), ".cache", "huggingface");
  try { return readFileSync(path.join(home, "token"), "utf8").trim(); } catch { return ""; }
}

async function hub(pathname) {
  const t = token();
  const res = await fetch(HF + pathname, { headers: t ? { authorization: `Bearer ${t}` } : {}, signal: AbortSignal.timeout(20_000) });
  if (res.status === 401 || res.status === 403) throw Object.assign(new Error(`Hugging Face says ${res.status}: the repo is gated or private. Accept its license on ${HF} and run \`hf auth login\`.`), { status: 403 });
  if (res.status === 404) throw Object.assign(new Error("not found on Hugging Face"), { status: 404 });
  if (!res.ok) throw new Error(`Hugging Face ${pathname} -> HTTP ${res.status}`);
  return res.json();
}

function summarize(m) {
  const tags = m.tags || [];
  return {
    repo: m.id, url: `${HF}/${m.id}`, author: m.author || m.id.split("/")[0], pipeline: m.pipeline_tag || null,
    downloads: m.downloads ?? 0, likes: m.likes ?? 0, trending: m.trendingScore ?? 0,
    gated: !!m.gated, updated: m.lastModified || null, created: m.createdAt || null,
    formats: FORMATS.filter((f) => tags.includes(f)),
    license: tags.find((t) => t.startsWith("license:"))?.slice(8) || null,
    kind: Object.entries(PIPELINES).find(([, p]) => p.includes(m.pipeline_tag))?.[0] || null,
    family: FAMILIES.find((f) => f.download.items.some((it) => it.repo === m.id))?.id || null,
  };
}

/** Split what was typed into a repo to look up directly (a pasted link or exact org/name), an author, and free text. */
export function parseQuery(raw = "") {
  const text = raw.trim();
  const link = REPO_LINK.exec(text)?.[1] || REPO_ID.exec(text)?.[1] || null;
  if (link) return { repo: link.replace(/\.git$/, ""), author: link.split("/")[0], search: link.split("/")[1] };
  // "org/partial name" searches within that org; "author:org words" too.
  const by = /^author:(\S+)\s*(.*)$/i.exec(text) || /^([\w.-]+)\/(.*)$/.exec(text);
  if (by) return { repo: null, author: by[1], search: by[2].trim() };
  return { repo: null, author: null, search: text };
}

/**
 * Search hub models. kind narrows to image / video / audio / chat pipelines (omit or "all" for any);
 * sort = downloads | likes | trending | modified | created; format = gguf | safetensors | diffusers.
 * A pasted Hugging Face link or exact org/name is always returned first, whatever the filters.
 */
export async function searchModels({ query = "", kind, limit = 20, sort = "downloads", format, author } = {}) {
  const sortKey = SORTS[sort] || "downloads";
  const tags = kind && kind !== "all" ? PIPELINES[kind] || [] : [null];
  const parsed = parseQuery(query);
  const seen = new Map();
  if (parsed.repo) {
    try { seen.set(parsed.repo, summarize(await hub(`/api/models/${parsed.repo}`))); } catch (e) { if (e.status !== 404) throw e; }
  }
  const lists = await Promise.all(tags.map((tag) => {
    const q = new URLSearchParams({ sort: sortKey, direction: "-1", limit: String(limit), full: "false" });
    if (parsed.search) q.set("search", parsed.search);
    if (author || parsed.author) q.set("author", author || parsed.author);
    if (format) q.set("filter", format);
    if (tag) q.set("pipeline_tag", tag);
    return hub(`/api/models?${q}`);
  }));
  const exact = parsed.repo ? seen.get(parsed.repo) : null;
  if (exact) seen.delete(parsed.repo);
  // Several pipeline tags mean several lists: merge them back into one order.
  const by = { downloads: "downloads", likes: "likes", trending: "trending", modified: "updated", created: "created" }[sort] || "downloads";
  for (const m of lists.flat()) if (!seen.has(m.id) && m.id !== parsed.repo) seen.set(m.id, summarize(m));
  const rest = [...seen.values()].sort((a, b) => (b[by] > a[by] ? 1 : b[by] < a[by] ? -1 : 0));
  return [...(exact ? [exact] : []), ...rest].slice(0, limit);
}

/** Weight files in a repo, with sizes and direct links. */
export async function repoFiles(repo) {
  const tree = await hub(`/api/models/${repo}/tree/main?recursive=true`);
  const info = await hub(`/api/models/${repo}`).catch(() => ({}));
  const files = tree
    .filter((e) => e.type === "file" && WEIGHT_EXT.has(path.extname(e.path).toLowerCase()))
    .map((e) => ({ file: e.path, size: e.lfs?.size ?? e.size, url: `${HF}/${repo}/blob/main/${e.path}` }))
    .sort((a, b) => a.file.localeCompare(b.file));
  const chat = PIPELINES.chat.includes(info.pipeline_tag);
  if (chat) for (const f of files) f.ollama = ollamaName(repo, f.file, files)?.name || null;
  return { repo, url: `${HF}/${repo}`, gated: !!info.gated, pipeline: info.pipeline_tag || null, chat, files, size: files.reduce((s, f) => s + (f.size || 0), 0) };
}

// Ollama refuses names whose repo or tag part is longer than this.
const OLLAMA_PART_MAX = 80;

/**
 * What a chat GGUF is called once in Ollama. direct: Ollama can pull it itself as hf.co/org/repo:QUANT (the whole
 * file name when two files share a quant). Repo names over Ollama's limit can't be pulled that way, so they get a
 * shortened org/repo:quant name and are downloaded, then imported. Null for files Ollama can't run on their own
 * (split GGUFs, vision projectors, files in subfolders).
 */
export function ollamaName(repo, file, files) {
  if (!/\.gguf$/i.test(file) || file.includes("/") || /mmproj|-\d{5}-of-\d{5}/i.test(file)) return null;
  const quant = (f) => /[-_.]((?:I?Q\d\w*?|F16|F32|BF16))\.gguf$/i.exec(f)?.[1]?.toUpperCase();
  const q = quant(file);
  const tag = q && files.filter((f) => quant(f.file) === q).length === 1 ? q : file;
  const [org, name] = repo.split("/");
  if (name.length <= OLLAMA_PART_MAX && tag.length <= OLLAMA_PART_MAX) return { name: `hf.co/${repo}:${tag}`, direct: true };
  const clip = (s) => s.toLowerCase().replace(/[^a-z0-9._-]/g, "-").slice(0, OLLAMA_PART_MAX).replace(/[-_.]+$/, "");
  return { name: `${clip(org)}/${clip(name)}:${clip(tag.replace(/\.gguf$/i, ""))}`, direct: false };
}

/** For a family id: the roles to download, i.e. only what the installed copy is missing (all of them if none is). */
export async function missingRoles(familyId) {
  const { models } = await listModels({ refresh: true });
  const installed = models.filter((m) => m.family === familyId);
  if (!installed.length) return null;
  const best = installed.find((m) => m.status === "ready") || installed[0];
  return [...best.missing, ...best.broken];
}
