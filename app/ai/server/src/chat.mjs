// Chat with local LLMs (Ollama). Conversations are stored in SQLite; replies stream to the client as
// newline-delimited JSON events: { type: "thinking" | "token", text } ... then { type: "done", message } or
// { type: "error", error }.
import { randomUUID } from "node:crypto";
import { OLLAMA_URL } from "./config.mjs";
import { listChats, getChat, createChat, updateChat, deleteChat, addMessage } from "./db.mjs";
import { comfyUp, freeComfy, unloadLlm } from "./comfy.mjs";
import { listLlms } from "./llm.mjs";
import { getCharacter, personaPrompt } from "./characters.mjs";

const TITLE_CHARS = 60;
const MAX_MESSAGE_CHARS = 200_000;
const SYSTEM_PROMPT = "You are DOGS AI, a helpful assistant running entirely on the user's own computer. " +
  "Be direct and concise. Use Markdown for structure and fenced code blocks for code.";

export { listChats, getChat, deleteChat };

const newId = () => randomUUID().slice(0, 12);

export async function newChat({ model, characterId } = {}) {
  const character = characterId ? await getCharacter(String(characterId)) : null;
  if (characterId && !character) throw Object.assign(new Error("no such character"), { status: 404 });
  const chat = { id: newId(), title: character ? `Chat with ${character.name}` : "New chat", model: model || null, created: Date.now(), characterId: character?.id };
  await createChat(chat);
  return { ...chat, character_id: chat.characterId ?? null, updated: chat.created, messages: [] };
}

// Loading a model needs large blocks of pinned RAM (and CUDA compiles kernels in RAM too). Without a page file
// Windows refuses those near its commit limit, which shows up as any of these. It is usually brief: memory from a
// crashed runner or a just-freed ComfyUI model comes back within seconds, so these are worth a retry.
const MEMORY_ERROR = /failed to allocate|out of memory|cudaMalloc|insufficient memory|unable to allocate|PTX JIT|ptxas/i;
const LOAD_RETRIES = 2;
const RETRY_WAIT_MS = 4000;

// Ollama's raw load errors are unreadable; the common one on this kind of machine is running out of memory.
function friendly(message) {
  if (MEMORY_ERROR.test(message)) {
    return "Not enough free memory to load the chat model. Close some apps (or wait for downloads and image jobs " +
      "to finish), pick a smaller model or quant, then try again.";
  }
  return message;
}

/** Make room for a model that failed to load: drop ComfyUI's models and any other loaded LLM, then give it a moment. */
async function makeRoom() {
  if (await comfyUp()) await freeComfy().catch(() => {});
  await unloadLlm().catch(() => {});
  await new Promise((r) => setTimeout(r, RETRY_WAIT_MS));
}

async function defaultModel() {
  const { models = [] } = await listLlms();
  // Prefer the fast (no thinking) alias set up by the installer, then anything installed.
  return models.find((m) => /fast/.test(m.name))?.name || models[0]?.name || null;
}

/**
 * Add a user message and stream the assistant's reply into `write` (one JSON object per call).
 * @param {string} chatId
 * @param {{ content: string, model?: string, think?: boolean }} body
 * @param {(event: object) => void} write
 * @param {AbortSignal} signal  aborted when the client disconnects or presses Stop
 */
export async function sendMessage(chatId, body, write, signal) {
  const chat = await getChat(chatId);
  if (!chat) throw Object.assign(new Error("no such chat"), { status: 404 });
  const content = String(body.content || "").trim();
  if (!content) throw Object.assign(new Error("message is empty"), { status: 400 });
  if (content.length > MAX_MESSAGE_CHARS) throw Object.assign(new Error("message is too long"), { status: 413 });
  const model = body.model || chat.model || (await defaultModel());
  if (!model) throw Object.assign(new Error("no chat model installed (ollama pull qwen3.5:9b)"), { status: 400 });

  const user = { id: newId(), chat_id: chatId, role: "user", content, created: Date.now() };
  await addMessage(user);
  const character = chat.character_id ? await getCharacter(chat.character_id) : null;
  if (chat.title === "New chat") await updateChat(chatId, { title: content.replace(/\s+/g, " ").slice(0, TITLE_CHARS) });
  await updateChat(chatId, { model, updated: user.created });

  // The GPU is shared with ComfyUI: drop its models so the LLM fits in VRAM.
  if (await comfyUp()) await freeComfy().catch(() => {});

  const system = character ? `${personaPrompt(character)}\n\n${SYSTEM_PROMPT.replace(/^You are DOGS AI, a helpful assistant/, "You are")}` : SYSTEM_PROMPT;
  const history = [{ role: "system", content: system }, ...chat.messages.map((m) => ({ role: m.role, content: m.content })), { role: "user", content }];
  let text = "", thinking = "", final = {};
  // A load that fails for lack of memory, before anything was streamed, is retried after making room.
  for (let attempt = 0; ; attempt++) {
    const retry = async (msg) => {
      if (attempt >= LOAD_RETRIES || text || thinking || signal.aborted || !MEMORY_ERROR.test(msg)) throw new Error(friendly(msg.slice(0, 600)));
      await makeRoom();
    };
    const res = await fetch(`${OLLAMA_URL}/api/chat`, {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ model, messages: history, stream: true, think: !!body.think }),
      signal,
    });
    if (!res.ok) {
      const raw = await res.text();
      let msg = raw;
      try { msg = JSON.parse(raw).error || raw; } catch {}
      await retry(msg);
      continue;
    }

    const decoder = new TextDecoder();
    let buf = "", failed = null;
    try {
      for await (const chunk of res.body) {
        buf += decoder.decode(chunk, { stream: true });
        let nl;
        while ((nl = buf.indexOf("\n")) >= 0) {
          const line = buf.slice(0, nl).trim();
          buf = buf.slice(nl + 1);
          if (!line) continue;
          const ev = JSON.parse(line);
          if (ev.error) { failed = ev.error; break; }
          if (ev.message?.thinking) { thinking += ev.message.thinking; write({ type: "thinking", text: ev.message.thinking }); }
          if (ev.message?.content) { text += ev.message.content; write({ type: "token", text: ev.message.content }); }
          if (ev.done) final = ev;
        }
        if (failed) break;
      }
    } catch (e) {
      if (!signal.aborted) throw e;
    }
    if (!failed) break;
    await retry(failed);
  }
  // Keep whatever arrived, even if the user pressed Stop.
  const reply = {
    id: newId(), chat_id: chatId, role: "assistant", content: text || (signal.aborted ? "_(stopped)_" : ""), thinking: thinking || null, model,
    tokens: final.eval_count ?? null,
    context: final.prompt_eval_count != null ? final.prompt_eval_count + (final.eval_count || 0) : null,
    created: Date.now(),
  };
  await addMessage(reply);
  await updateChat(chatId, { updated: reply.created });
  write({ type: "done", message: reply, stopped: signal.aborted, seconds: final.total_duration ? final.total_duration / 1e9 : null });
  return reply;
}
