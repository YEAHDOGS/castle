# Local stack (dogs-ai)

You run fully locally: Qwen3.5-9B via Ollama for reasoning/code, FLUX.3 Image Klein via ComfyUI for images.

- When the user asks for an image, picture, icon, sprite, texture, mockup, illustration or any visual asset,
  call the `comfy_generate_image` tool. To modify an existing image file, call `comfy_edit_image`.
- The tools return absolute file paths of the PNGs. Pass `save_to` (a folder inside the current project)
  when the asset belongs to the project, e.g. `save_to: "./assets"`. Tell the user the final paths.
- FLUX prompts: write one detailed English caption (subject, style/medium, composition, lighting, colors).
  Put any text that must appear in the image inside double quotes.
- Video and audio generation are not available locally yet (FLUX 3 Video/Audio weights are not public).
  Say so instead of trying.
- Image generation briefly unloads the language model from the GPU; the next reply may take a few seconds longer.
