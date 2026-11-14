# dogs-ai: local text, image, video and audio generation

Everything runs on your own machine: models run in **ComfyUI** on your GPU, the **`dogs` CLI** and **API** drive
it, and the **DOGS AI** app (Svelte + Tailwind in a Tauri shell) puts it behind a prompt box. The
same API is what an outside assistant calls later ("I need an image of skyscrapers" -> PNG back).

```
 DOGS AI app (desktop / browser)      dogs CLI          other tools (Muse, scripts, curl)
              \                          |                       /
               +------------ dogs API  :8787 (server/) --------+
                                         |
                              ComfyUI :8188 (runtime/ComfyUI)  ->  outputs/
                                         |
         models: ./models  +  every user's ~/.cache/huggingface/hub  +  runtime/models
```

| Piece | What | Where |
|---|---|---|
| `dogs` CLI + API | Scans models, queues jobs, talks to ComfyUI, downloads from Hugging Face | `server/` (Node 22+, no dependencies) |
| DOGS AI app | Prompt, pick a model, see results; desktop app or web page | `app/` (Svelte 5, Tailwind 4, Tauri 2) |
| ComfyUI `v0.42.5` | Runs the models | `runtime/ComfyUI` (git-ignored) |
| opencode + Qwen3.5 (Ollama) | Agentic coding with local models (see [Coding agent](#coding-agent-opencode)) | global, `opencode/`, `mcp/comfy-mcp` |

## Install (Windows)

```powershell
git clone <this repo> dogs-ai
cd dogs-ai
powershell -ExecutionPolicy Bypass -File .\install.ps1
```

The installer is idempotent. It installs git, Node.js, uv and Ollama if missing, ComfyUI with CUDA PyTorch,
opencode, the `dogs` command (`npm link`), and the app's dependencies, then downloads the default models.
Flags: `-SkipModels`, `-SkipOpencodeConfig`. Health check: `.\scripts\doctor.ps1`.

Already have the repo and ComfyUI? Just the new parts:

```powershell
npm link                 # puts `dogs` on your PATH
npm run app:install      # app + Tauri CLI (the desktop app also needs Rust, see below)
```

## Generate an image from one command

```powershell
dogs gen image "skyscrapers at dusk, wet streets reflecting neon"
```

It prints the path of the PNG (in `outputs\generated\image\`). ComfyUI starts by itself on the first run (about 30 s), and
the first image takes longer while the model loads. More:

```powershell
dogs gen image "a beagle astronaut" --width 1344 --height 768 --steps 12 --seed 42 --out .\assets
dogs gen video "a drone shot over a neon city at night" --seconds 2
dogs gen audio "lo-fi hip hop, warm rhodes, vinyl crackle" --seconds 30 --lyrics "[instrumental]"
dogs gen image "..." --model "flux3-klein:flux3-image-klein-int8-convrot.safetensors"   # pick a specific model
```

Any model parameter works as a `--flag` (see `dogs models --json` for each model's `params`). If `dogs serve` is
running, `dogs gen` queues through it (so the app sees the job too), otherwise it runs in-process.

### Image to video (Animate)

Any image can become a short video in one click. In the app, every generated image (in the Library, under
results in Create, and in the full-screen view) has an **Animate** button. Click it and the video renders right
next to the image, with a progress ring while it works. The caret beside it opens an optional **motion prompt**
("hair blowing in the wind, slow zoom out") and a link to Create → Video for every setting.

From a terminal or another tool:

```powershell
dogs animate .\outputs\generated\image\flux3-klein_00001_.png                      # one step, sensible defaults
dogs animate .\photo.jpg "the camera slowly pushes in, rain falls" --seconds 3     # any image, with a motion prompt
curl -s http://127.0.0.1:8787/api/animate -H "content-type: application/json" -d '{"image":"generated/image/flux3-klein_00001_.png","motion":"slow zoom out","wait":true}'
```

Animate picks the installed video model that takes a start frame (Wan 2.2 TI2V 5B first, then LTX-Video),
matches the image's aspect ratio at about 640×640 pixels, 2 seconds, and uses the motion text plus the prompt the
image was made with. The video job records the image as `params.start_image`, which is how the app shows it next
to the image. `image` is a path under `outputs/` or a `data:image/...;base64,` URL. Any video parameter
(`seconds`, `steps`, `width`, ...) can be passed to override the defaults.

**Model setup:** Wan 2.2 TI2V 5B is the default (`dogs download wan2.2-ti2v-5b`, 16.9 GB; LTX-Video also works).
On an RTX 4050 6 GB / 32 GB RAM laptop a one-click Animate (640×640, 2 s, 20 steps) takes a few minutes. Wan loads
its weights as **fp8** by default (`weights` parameter, `fp16` for full precision): in fp16 the model plus its
text encoder stage about 17 GB in system RAM, which ran Windows out of memory on a 32 GB machine **without a
page file** and crashed ComfyUI mid-render. With fp16, or on smaller machines, keep a system-managed page file.

The longer form still works: `dogs gen video "<prompt>" --image photo.png`, Create → Video's **Start frame**
drop zone, or `"start_image"` on `/api/generate`. With a start frame and no width/height given, the video takes
the image's aspect ratio.

## The CLI

```
dogs serve [--host 0.0.0.0] [--port 8787]   API + the web app at http://host:port/
dogs app                                    open the desktop app
dogs models [--kind image|video|audio]      what is installed, ready, or missing (and the download size)
dogs where                                  every folder scanned for models
dogs llm                                    local LLMs (Ollama): size, context window (configured / max), loaded state
dogs download                               list downloadable model families with sizes and HF links
dogs download <family>                      fetch only the files that family is missing (--all for every file)
dogs download <org/repo> [file ...]         any Hugging Face repo or file
dogs hf search [words] --kind image         search Hugging Face (also --sort likes|trending|modified, --limit)
dogs hf files <org/repo>                    weight files in a repo, with sizes and links
dogs hf <anything>                          straight to the hf CLI: dogs hf auth login, dogs hf cache ls, ...
dogs gen <image|video|audio> "<prompt>"     generate
dogs animate <image> ["motion prompt"]      turn an image into a short video (see Image to video)
dogs jobs                                   recent generations and their files
dogs comfy start|stop|restart|status        the ComfyUI process
dogs comfy add-node gguf                    ComfyUI-GGUF, needed for .gguf weights
dogs models import <model id>               copy a model in when its files are on another drive
```

### Terminal tools this project uses

| Command | For |
|---|---|
| `dogs` | everything above |
| `hf` (or `uvx --from huggingface_hub hf`) | Hugging Face login and downloads (`dogs download` / `dogs hf` wrap it) |
| `opencode` | the local coding agent |
| `ollama` | runs the Qwen chat model for opencode |
| ComfyUI web UI, <http://127.0.0.1:8188> | building or exporting workflows by hand |

## Models

### Where models are found

`dogs` scans, on every refresh:

1. **`models/`** in this project. Drop plain `.safetensors` / `.gguf` files here, or cut and paste whole
   Hugging Face cache folders (`models--org--name`).
2. **Every user profile's Hugging Face cache**: `C:\Users\*\.cache\huggingface\hub` (plus `HF_HOME` /
   `HF_HUB_CACHE`). `hf download` saves into the cache of whoever ran it, often an admin account, so all of
   them are checked. This account needs read access to that folder.
3. `runtime/models` and ComfyUI's own `runtime/ComfyUI/models`.

Hugging Face caches store files as links into a `blobs/` folder. If you copy a `models--...` folder without
its blobs, the links break. `dogs` then looks for the blob in every other cache (the original is usually still
in the admin's cache) and shows the file as **broken** only if it can't find it anywhere.

Files that ComfyUI needs are hard-linked (no extra disk space) into `runtime/models/linked/`, which ComfyUI
reads through `runtime/extra_model_paths.yaml`. Hard links only work on the same drive: for a model on another
drive run `dogs models import <id>` to copy it.

### A model is more than one file

Most models need three files: the diffusion weights, a **text encoder** and a **VAE**. Copying only the big
weights file gives you an **incomplete** model. `dogs models` and the app's Models panel show what is missing,
its size, and a direct link. `dogs download <family>` fetches just those files.

### Built-in model families

| Family | Kind | Full download | Notes |
|---|---|---|---|
| `flux3-klein` | image | 10.3 GB | FLUX.3 Image Klein. Gated: accept the license on Hugging Face first |
| `qwen-image-2.1` | image | 16.1 GB | Qwen-Image 2.1. `.gguf` quants need `dogs comfy add-node gguf` |
| `wan2.2-ti2v-5b` | video | 16.9 GB | Wan 2.2 5B, text or image to video, 720p/24 fps. Default for Animate. Loads as fp8 |
| `ltx-video-2b` | video | 13.3 GB | LTX-Video 2B, smaller and faster |
| `ace-step-1.5` | audio | 9.3 GB | ACE-Step 1.5 songs/instrumentals, optional lyrics |
| `stable-audio-open` | audio | 5.4 GB | Stable Audio Open: sound effects and loops up to 47 s |

Their ComfyUI graphs live in [`server/src/families.mjs`](server/src/families.mjs), following the official
ComfyUI templates. Gated repos: run `dogs hf auth login` once.

### Any other model: bring a workflow

Build the workflow in the ComfyUI web UI, export it with **Export (API)**, and save it as
`workflows/<name>.json`. Add a `"dogs"` key and use placeholders where the prompt and settings go:

```json
{
  "dogs": { "kind": "image", "label": "My SDXL", "params": { "steps": { "type": "int", "default": 30, "min": 1, "max": 80, "step": 1 } } },
  "3": { "class_type": "KSampler", "inputs": { "seed": "{{seed}}", "steps": "{{steps}}", "...": "..." } },
  "6": { "class_type": "CLIPTextEncode", "inputs": { "text": "{{prompt}}", "clip": ["4", 1] } }
}
```

Placeholders: `{{prompt}}`, `{{seed}}`, `{{prefix}}` (output filename prefix), and any key from `params`. It
shows up as a model of that kind in the CLI, the API and the app.

## The app

Two tabs:

- **Create**: tap it to open, then pick Image / Video / Audio at the top. Or **press and hold** Create: a
  picker slides up, slide onto a kind and let go. Tapping Create while you're already on it opens the picker too.
- **Library**, in expandable sections:
  - **Download models**: every model family not installed yet (or missing files), with sizes, direct
    Hugging Face links, and Hugging Face search.
  - **Models**: a storage bar (GB used by model files, per location, plus Ollama, against free disk space) and
    large tiles for every model, Ollama LLMs included, with their context window. One row shows until you
    expand. Hover a tile to delete it. The warning lists the exact files and keeps any file another model
    still uses.
  - **Generated**: everything you've made, newest first, grouped by day, with time, prompt, model and seed.
    Search prompts, filter by kind, and **Convert**, **Animate** (images: one click, videos appear next to the
    image), save, copy the prompt, or delete.

**Converter** (Library → Convert) runs entirely in the app, with no upload and no server work:

| Source | Exports | Options |
|---|---|---|
| Image | PNG, JPG, WebP, ICO (AVIF where the browser can encode it) | size (aspect lock), quality, **target file size** |
| Audio | MP3, M4A (AAC), OGG / WebM (Opus), WAV | bitrate, sample rate, mono/stereo, **target file size** |
| Video | MP4, WebM, MOV, MKV, or just the soundtrack as audio | resolution, bitrate, **target file size** |

Target file size searches image quality (then dimensions) or sets the bitrate from the length, so the file
lands just under it. If the target is below what the encoder allows (Windows' AAC needs 96 kbps or more,
for example), it gives you the smallest possible file and says so. Images go through the canvas encoder;
audio and video through [Mediabunny](https://mediabunny.dev) (WebCodecs), which works the same in a
browser and the desktop app's webview. Saving uses the browser download; the desktop app shows the system
Save dialog.

### Why there is an engine (server)

The app is a web page, and a web page can't read your disks, scan other users' caches, run ComfyUI (Python)
or write a database. The **engine** (`dogs serve`, Node) does those, locally. The desktop app starts it
when it opens and restarts it if it stops, so you never start it by hand. While it comes up the app shows
"Starting engine…" and keeps retrying. Only the browser/website mode needs `dogs serve` running yourself.

### Data

Generation history (prompt, model, parameters, seed, timestamps, output files) lives in **SQLite** at
`runtime/dogs.db`, through Node's built-in `node:sqlite` (no dependency, no database server; also works on a
Raspberry Pi). Every generated file (app, CLI, API and the opencode `/image` tools) is a plain file in
`outputs/generated/<image|video|audio>/` in this project folder; `outputs/inputs/` holds uploaded start frames and
`outputs/characters/` character uploads. Files from older versions in `outputs/<kind>/` are moved there (and
their history updated) the next time the engine starts. All database access is in `server/src/db.mjs` with plain SQL, so a
later move to Cloudflare Workers + D1 (also SQLite) swaps that one file, and `outputs/` maps to R2. There is no
ORM; the schema is one table.

| How | Command |
|---|---|
| Desktop window (dev, hot reload) | `npm run desktop` (or `dogs app`) |
| In a browser, served by the API | `npm run web`, then open <http://127.0.0.1:8787> |
| In a browser, hot reload | `dogs serve` in one terminal, `npm --prefix app run dev` in another |

The desktop app starts `dogs serve` itself if nothing is listening on port 8787. Settings (gear icon) point
it at another machine's API, with an API key if that server has one.

### Build the desktop app

The desktop app is a [Tauri 2](https://tauri.app) shell, so building it needs Rust as well as Node:

- **Windows:** [rustup](https://rustup.rs) plus the MSVC C++ build tools ("Desktop development with C++" in the
  Visual Studio Build Tools). WebView2 ships with Windows 10/11.
- **macOS:** rustup plus the Xcode command line tools (`xcode-select --install`).
- **Linux:** rustup plus `libwebkit2gtk-4.1-dev libappindicator3-dev librsvg2-dev patchelf libxdo-dev` (Debian/Ubuntu names).

Then, from the repo root:

```powershell
npm run app:install     # once
npm run dist            # -> app\src-tauri\target\release\bundle\
```

| Output | What |
|---|---|
| `bundle\nsis\DOGS AI_<version>_x64-setup.exe` | Installer. Asks for admin and installs **per machine** into `C:\Program Files\DOGS AI\`, with a Start menu shortcut and an uninstaller |
| `bundle\msi\DOGS AI_<version>_x64_en-US.msi` | The same as an MSI |
| `target\release\dogs-ai.exe` | The bare app, no install (also from `npm run dist:dir`, which is faster) |

On a Mac the same command makes `bundle/dmg/DOGS AI_<version>_aarch64.dmg` and `bundle/macos/DOGS AI.app`
(Apple's tools are required, so it can't be cross-built from Windows). Open the .dmg and drag **DOGS AI** to
**Applications**. The build is not signed, so the first launch needs right-click > **Open** (or
`xattr -cr "/Applications/DOGS AI.app"`). On Linux it makes an AppImage and a .deb.

The engine is Node, which the desktop app does not include: it uses the Node in the `dogs` CLI archive
(`node/`), else the one on your PATH.

Nothing here runs in CI on purpose: builds stay local, and nothing is uploaded (`publish` is off).

The packaged app runs the `dogs` server from this project folder. The path is recorded at build time; set
`DOGS_HOME` to override it if you move the folder.

## API (for other tools)

`dogs serve` listens on `127.0.0.1:8787`. To reach it from other machines (a Raspberry Pi on the router, an
assistant), bind it to the network **with a key**:

```powershell
$env:DOGS_API_KEY = "<long random string>"
dogs serve --host 0.0.0.0
```

Clients send `Authorization: Bearer <key>` (or `?key=` for `<img src>` / EventSource).

```bash
# one call, waits, returns the finished job with file URLs
curl -s http://127.0.0.1:8787/api/generate -H "content-type: application/json" \
  -d '{"kind":"image","prompt":"skyscrapers at dusk","width":1024,"height":768,"wait":true}'
# -> { "status":"done", "outputs":[{ "url":"/files/image/flux3-klein_00003_.png", "mime":"image/png" }], ... }
curl -s -o skyscrapers.png http://127.0.0.1:8787/files/image/flux3-klein_00003_.png
```

`GET /api` lists every endpoint: models (with storage totals, and delete), generate, jobs (poll, search prompts
with `?q=`, cancel or delete), `/api/events` (live updates as Server-Sent Events), Ollama LLMs (context windows,
unload, delete), Hugging Face search/files/downloads, and ComfyUI start/stop/free. Jobs run one at a time,
because there is one GPU.

## Coding agent (opencode)

Unchanged from before: `.\scripts\start.ps1`, then `opencode` in any project. The default model is
`ollama/qwen3.5-9b-fast`; the `plan` agent (Tab) uses `qwen3.5-9b-agent` with thinking on. `/image` and
`/edit-image` use FLUX through `mcp/comfy-mcp`. On a 6 GB GPU the MCP server unloads Qwen before sampling and
frees VRAM afterwards (`VRAM_HANDOFF=0` turns that off); `dogs` does the same.

## Layout

```
install.ps1, stack.json     installer and pinned versions
server/                     dogs CLI (bin/dogs.mjs) and API (src/): scan, families, comfy, jobs, db, llm, hub, api
app/                        DOGS AI app: src/ (Svelte), src-tauri/ (Tauri desktop shell, Rust)
workflows/                  your own ComfyUI API-format workflows (optional)
models/                     drop models here (git-ignored)
mcp/comfy-mcp, opencode/, ollama/, scripts/   coding agent setup and PowerShell helpers
runtime/, outputs/          ComfyUI, linked models, database; outputs/generated/ has every generated file (git-ignored)
```
