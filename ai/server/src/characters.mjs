// Characters: a name, a bio, any number of named lists (hobbies, favorite movies, ...), and media — generated in
// this app or dropped in from outside. Uploaded files live in outputs/characters/<id>/; generated ones are
// referenced where they already are. A character can also be the persona for a chat (see chat.mjs).
import { promises as fs, createWriteStream } from "node:fs";
import path from "node:path";
import { randomUUID } from "node:crypto";
import { pipeline } from "node:stream/promises";
import { OUTPUTS } from "./config.mjs";
import {
  listCharacters, getCharacter, saveCharacter, deleteCharacterRow, addCharacterMedia, getCharacterMedia, deleteCharacterMediaRow,
} from "./db.mjs";

export { listCharacters, getCharacter };

const MAX_UPLOAD_BYTES = 2 * 2 ** 30; // 2 GB per file
const MAX_NAME = 120, MAX_BIO = 20_000, MAX_LISTS = 50, MAX_ITEMS = 500, MAX_ITEM = 500;
const KIND_BY_EXT = {
  png: "image", jpg: "image", jpeg: "image", webp: "image", gif: "image", avif: "image",
  mp4: "video", webm: "video", mov: "video", mkv: "video",
  mp3: "audio", wav: "audio", flac: "audio", ogg: "audio", m4a: "audio", opus: "audio",
};
const bad = (msg, status = 400) => Object.assign(new Error(msg), { status });
const newId = () => randomUUID().slice(0, 12);
const clip = (s, n) => String(s ?? "").slice(0, n);

/** Keep only the shape we expect: [{ id, title, items: [string] }]. */
function cleanLists(lists) {
  if (!Array.isArray(lists)) return [];
  return lists.slice(0, MAX_LISTS).map((l) => ({
    id: typeof l?.id === "string" && /^[\w-]{1,24}$/.test(l.id) ? l.id : newId(),
    title: clip(l?.title, MAX_NAME) || "List",
    items: (Array.isArray(l?.items) ? l.items : []).slice(0, MAX_ITEMS).map((x) => clip(x, MAX_ITEM)).filter((x) => x.trim()),
  }));
}

// Lists saved before ids were checked properly have none, and the editor needs them as keys.
for (const c of await listCharacters()) {
  if (c.lists.some((l) => typeof l.id !== "string")) await saveCharacter({ ...c, lists: cleanLists(c.lists) });
}

export async function createCharacter(body = {}) {
  const now = Date.now();
  const c = {
    id: newId(), name: clip(body.name, MAX_NAME).trim() || "New character", bio: clip(body.bio, MAX_BIO),
    lists: cleanLists(body.lists ?? [{ title: "Hobbies", items: [] }, { title: "Favorite movies", items: [] }, { title: "Favorite books", items: [] }]),
    avatar: null, created: now, updated: now,
  };
  await saveCharacter(c);
  return getCharacter(c.id);
}

export async function updateCharacter(id, body = {}) {
  const c = await getCharacter(id);
  if (!c) throw bad("no such character", 404);
  const next = {
    ...c,
    name: body.name !== undefined ? clip(body.name, MAX_NAME).trim() || c.name : c.name,
    bio: body.bio !== undefined ? clip(body.bio, MAX_BIO) : c.bio,
    lists: body.lists !== undefined ? cleanLists(body.lists) : c.lists,
    // The avatar must be one of the character's own media items.
    avatar: body.avatar !== undefined ? (body.avatar && c.media.some((m) => m.id === body.avatar) ? body.avatar : null) : c.avatar,
    updated: Date.now(),
  };
  await saveCharacter(next);
  return getCharacter(id);
}

export async function deleteCharacter(id) {
  const c = await getCharacter(id);
  if (!c) return null;
  await deleteCharacterRow(id);
  // Uploaded files belong to the character; generated media stays in the Library.
  await fs.rm(path.join(OUTPUTS, "characters", id), { recursive: true, force: true });
  return c;
}

function safeName(name) {
  const base = path.basename(String(name || "file")).replace(/[^\w.-]+/g, "_").replace(/^\.+/, "").slice(-100);
  return base || "file";
}

/** Attach a file that already exists under outputs/ (e.g. a generated image). */
export async function attachExisting(id, rel) {
  if (!(await getCharacter(id))) throw bad("no such character", 404);
  const clean = String(rel || "").replace(/\\/g, "/").replace(/^\/?files\//, "");
  const abs = path.resolve(OUTPUTS, clean);
  if (!abs.startsWith(OUTPUTS + path.sep)) throw bad("file must be under outputs/");
  try { await fs.access(abs); } catch { throw bad(`file not found: ${clean}`, 404); }
  const kind = KIND_BY_EXT[path.extname(abs).slice(1).toLowerCase()];
  if (!kind) throw bad("only images, video and audio can be attached");
  const m = { id: newId(), character_id: id, file: clean, kind, source: "generated", name: path.basename(clean), created: Date.now() };
  await addCharacterMedia(m);
  return m;
}

/** Stream an uploaded file (raw request body) into outputs/characters/<id>/. */
export async function attachUpload(id, req, filename) {
  if (!(await getCharacter(id))) throw bad("no such character", 404);
  const name = safeName(decodeURIComponent(filename || ""));
  const kind = KIND_BY_EXT[path.extname(name).slice(1).toLowerCase()];
  if (!kind) throw bad("only images, video and audio can be added (png, jpg, webp, gif, mp4, webm, mov, mp3, wav, ...)");
  const length = Number(req.headers["content-length"] || 0);
  if (length > MAX_UPLOAD_BYTES) throw bad("file is larger than 2 GB", 413);
  const dir = path.join(OUTPUTS, "characters", id);
  await fs.mkdir(dir, { recursive: true });
  const fileName = `${Date.now().toString(36)}_${name}`;
  const abs = path.join(dir, fileName);
  let seen = 0;
  try {
    await pipeline(req, async function* (src) {
      for await (const chunk of src) {
        seen += chunk.length;
        if (seen > MAX_UPLOAD_BYTES) throw bad("file is larger than 2 GB", 413);
        yield chunk;
      }
    }, createWriteStream(abs));
  } catch (e) {
    await fs.rm(abs, { force: true });
    throw e;
  }
  const m = { id: newId(), character_id: id, file: `characters/${id}/${fileName}`, kind, source: "upload", name, created: Date.now() };
  await addCharacterMedia(m);
  return m;
}

export async function removeMedia(id, mediaId) {
  const m = await getCharacterMedia(mediaId);
  if (!m || m.character_id !== id) throw bad("no such media", 404);
  await deleteCharacterMediaRow(mediaId);
  if (m.source === "upload") {
    const abs = path.resolve(OUTPUTS, m.file);
    if (abs.startsWith(path.join(OUTPUTS, "characters") + path.sep)) await fs.rm(abs, { force: true });
  }
  const c = await getCharacter(id);
  if (c?.avatar === mediaId) await updateCharacter(id, { avatar: null });
  return m;
}

/** A system prompt that lets an LLM speak as this character. */
export function personaPrompt(c) {
  const lists = c.lists.filter((l) => l.items.length).map((l) => `${l.title}: ${l.items.join(", ")}`).join("\n");
  return [`You are ${c.name}. Stay in character and speak as ${c.name} would.`, c.bio && `About you:\n${c.bio}`, lists && lists]
    .filter(Boolean).join("\n\n");
}
