// Paths and settings shared by the CLI and the API server. Everything comes from stack.json plus env overrides.
import { readFileSync, readdirSync } from "node:fs";
import path from "node:path";
import os from "node:os";
import { fileURLToPath } from "node:url";

export const REPO = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../..");
export const STACK = JSON.parse(readFileSync(path.join(REPO, "stack.json"), "utf8"));

const env = process.env;
export const IS_WIN = process.platform === "win32";
export const RUNTIME = path.join(REPO, "runtime");
export const COMFY_DIR = path.join(RUNTIME, "ComfyUI");
export const MODELS_DIR = path.resolve(env.DOGS_MODELS || path.join(REPO, "models"));
// Hard links to every scanned model file, grouped by ComfyUI folder. ComfyUI reads it via extra_model_paths.yaml.
export const LINKED_DIR = path.join(RUNTIME, "models", "linked");
export const EXTRA_PATHS_YAML = path.join(RUNTIME, "extra_model_paths.yaml");
export const OUTPUTS = path.resolve(env.DOGS_AI_OUTPUTS || path.join(REPO, "outputs"));
// Every generated file lands in outputs/generated/<kind>/; outputs/ also holds uploads (inputs/, characters/).
export const GENERATED = "generated";
export const WORKFLOWS_DIR = path.join(REPO, "workflows");
export const COMFY_URL = (env.COMFY_URL || `http://127.0.0.1:${STACK.comfyui.port}`).replace(/\/$/, "");
export const OLLAMA_URL = (env.OLLAMA_URL || STACK.ollama.host).replace(/\/$/, "");
export const VRAM_HANDOFF = env.VRAM_HANDOFF !== "0";

export const API_HOST = env.DOGS_HOST || "127.0.0.1";
export const API_PORT = Number(env.DOGS_PORT || STACK.api?.port || 8787);
// Required as "Authorization: Bearer <key>" when set. Always set one before binding to anything but localhost.
export const API_KEY = env.DOGS_API_KEY || "";

// Every Hugging Face hub cache on this machine: env overrides, then each user profile (models can be
// downloaded as any user, including an admin account).
export function hubRoots() {
  const roots = new Set();
  if (env.HF_HUB_CACHE) roots.add(env.HF_HUB_CACHE);
  if (env.HF_HOME) roots.add(path.join(env.HF_HOME, "hub"));
  roots.add(path.join(os.homedir(), ".cache", "huggingface", "hub"));
  const homes = IS_WIN
    ? [path.join(path.parse(os.homedir()).root, "Users")]
    : ["/home", "/Users"];
  for (const base of homes) {
    for (const n of readdirSafe(base)) roots.add(path.join(base, n, ".cache", "huggingface", "hub"));
  }
  if (!IS_WIN) roots.add("/root/.cache/huggingface/hub");
  return [...roots].map((r) => path.resolve(r));
}

const NOT_PROFILES = new Set(["Default", "Default User", "All Users"]);
function readdirSafe(dir) {
  try {
    return readdirSync(dir, { withFileTypes: true })
      .filter((e) => e.isDirectory() && !NOT_PROFILES.has(e.name))
      .map((e) => e.name);
  } catch { return []; }
}
