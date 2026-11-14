// Model families: which files make up a runnable model, where ComfyUI expects them, how to download them,
// which parameters they take, and the ComfyUI API-format graph that runs them. Graphs follow the official
// Comfy-Org workflow templates shipped with ComfyUI (comfyui_workflow_templates_json), flattened to API format.

const int = (def, min, max, step = 1) => ({ type: "int", default: def, min, max, step });
const num = (def, min, max, step = 0.1) => ({ type: "float", default: def, min, max, step });
const text = (def = "", label) => ({ type: "text", default: def, label });
const pick = (def, options) => ({ type: "select", default: def, options });
// An input image (outputs-relative path after upload). Empty = not used.
const image = (label) => ({ type: "image", default: "", label });

const IMAGE_SIZE = { width: int(1024, 256, 2048, 16), height: int(1024, 256, 2048, 16) };

// SaveVideo / SaveAudioAdvanced use V3 DynamicCombo inputs: children are passed as "<input>.<child>".
const saveVideo = (video, prefix) => ({ class_type: "SaveVideo", inputs: { video, filename_prefix: prefix, format: "mp4", "format.codec": "h264" } });
const saveAudio = (audio, prefix) => ({ class_type: "SaveAudioAdvanced", inputs: { audio, filename_prefix: prefix, format: "mp3", "format.quality": "V0" } });

export const FAMILIES = [
  {
    id: "flux3-klein",
    kind: "image",
    label: "FLUX.3 Image Klein",
    blurb: "Fast distilled text-to-image from Black Forest Labs. Good default on 6-8 GB GPUs.",
    files: {
      model: { folder: "diffusion_models", match: /^flux3-image-klein.*\.safetensors$/i },
      text_encoder: { folder: "text_encoders", match: /^qwen3vl_4b.*\.safetensors$/i },
      vae: { folder: "vae", match: /^flux2-vae.*\.safetensors$/i },
    },
    download: {
      gated: "https://huggingface.co/black-forest-labs/flux-3-image-klein-comfy",
      items: [
        { repo: "black-forest-labs/flux-3-image-klein-comfy", file: "diffusion_models/flux3-image-klein-int8-convrot.safetensors", role: "model", size: 5827874779 },
        { repo: "black-forest-labs/flux-3-image-klein-comfy", file: "text_encoders/qwen3vl_4b_int8_convrot.safetensors", role: "text_encoder", size: 4858559240 },
        { repo: "black-forest-labs/flux-3-image-klein-comfy", file: "vae/flux2-vae.safetensors", role: "vae", size: 336211292 },
      ],
    },
    params: { ...IMAGE_SIZE, steps: int(16, 4, 30), count: int(1, 1, 4) },
    graph: ({ files, prompt, width, height, steps, count, seed, prefix }) => ({
      unet: { class_type: "UNETLoader", inputs: { unet_name: files.model, weight_dtype: "default" } },
      clip: { class_type: "CLIPLoader", inputs: { clip_name: files.text_encoder, type: "flux3", device: "default" } },
      vae: { class_type: "VAELoader", inputs: { vae_name: files.vae } },
      pos: { class_type: "CLIPTextEncode", inputs: { text: prompt, clip: ["clip", 0] } },
      neg: { class_type: "CLIPTextEncode", inputs: { text: "", clip: ["clip", 0] } },
      latent: { class_type: "EmptyFlux2LatentImage", inputs: { width, height, batch_size: count } },
      sample: {
        class_type: "KSampler",
        inputs: { model: ["unet", 0], positive: ["pos", 0], negative: ["neg", 0], latent_image: ["latent", 0], seed, steps, cfg: 1, sampler_name: "euler", scheduler: "simple", denoise: 1 },
      },
      decode: { class_type: "VAEDecode", inputs: { samples: ["sample", 0], vae: ["vae", 0] } },
      save: { class_type: "SaveImage", inputs: { images: ["decode", 0], filename_prefix: prefix } },
    }),
  },
  {
    id: "flux2-klein-4b",
    kind: "image",
    label: "FLUX.2 Klein 4B",
    blurb: "Distilled 4-step FLUX.2. Very fast, ungated, and shares the FLUX VAE you may already have.",
    files: {
      model: { folder: "diffusion_models", match: /^flux-2-klein-4b.*\.safetensors$/i },
      text_encoder: { folder: "text_encoders", match: /^qwen_3_4b.*\.safetensors$/i },
      vae: { folder: "vae", match: /^flux2-vae.*\.safetensors$/i },
    },
    download: {
      items: [
        { repo: "Comfy-Org/flux2-klein", file: "split_files/diffusion_models/flux-2-klein-4b.safetensors", role: "model", size: 7751105712 },
        { repo: "Comfy-Org/flux2-klein", file: "split_files/text_encoders/qwen_3_4b.safetensors", role: "text_encoder", size: 8044982048 },
        { repo: "Comfy-Org/flux2-dev", file: "split_files/vae/flux2-vae.safetensors", role: "vae", size: 336213556 },
      ],
    },
    params: { ...IMAGE_SIZE, steps: int(4, 1, 16) },
    graph: ({ files, prompt, width, height, steps, seed, prefix }) => ({
      unet: { class_type: "UNETLoader", inputs: { unet_name: files.model, weight_dtype: "default" } },
      clip: { class_type: "CLIPLoader", inputs: { clip_name: files.text_encoder, type: "flux2", device: "default" } },
      vae: { class_type: "VAELoader", inputs: { vae_name: files.vae } },
      pos: { class_type: "CLIPTextEncode", inputs: { text: prompt, clip: ["clip", 0] } },
      neg: { class_type: "ConditioningZeroOut", inputs: { conditioning: ["pos", 0] } },
      guider: { class_type: "CFGGuider", inputs: { model: ["unet", 0], positive: ["pos", 0], negative: ["neg", 0], cfg: 1 } },
      noise: { class_type: "RandomNoise", inputs: { noise_seed: seed } },
      sampler: { class_type: "KSamplerSelect", inputs: { sampler_name: "euler" } },
      sigmas: { class_type: "Flux2Scheduler", inputs: { steps, width, height } },
      latent: { class_type: "EmptyFlux2LatentImage", inputs: { width, height, batch_size: 1 } },
      sample: { class_type: "SamplerCustomAdvanced", inputs: { noise: ["noise", 0], guider: ["guider", 0], sampler: ["sampler", 0], sigmas: ["sigmas", 0], latent_image: ["latent", 0] } },
      decode: { class_type: "VAEDecode", inputs: { samples: ["sample", 0], vae: ["vae", 0] } },
      save: { class_type: "SaveImage", inputs: { images: ["decode", 0], filename_prefix: prefix } },
    }),
  },
  {
    id: "flux2-klein-9b",
    kind: "image",
    label: "FLUX.2 Klein 9B",
    blurb: "Bigger, undistilled FLUX.2 (20 steps). Better detail; wants 12 GB+ VRAM.",
    files: {
      model: { folder: "diffusion_models", match: /^flux-2-klein-base-9b.*\.safetensors$/i },
      text_encoder: { folder: "text_encoders", match: /^qwen_3_8b.*\.safetensors$/i },
      vae: { folder: "vae", match: /^(full_encoder_small_decoder|flux2-vae).*\.safetensors$/i },
    },
    download: {
      gated: "https://huggingface.co/black-forest-labs/FLUX.2-klein-base-9b-fp8",
      items: [
        // Size unknown until the license is accepted on Hugging Face.
        { repo: "black-forest-labs/FLUX.2-klein-base-9b-fp8", file: "flux-2-klein-base-9b-fp8.safetensors", role: "model", size: null },
        { repo: "Comfy-Org/flux2-klein-9B", file: "split_files/text_encoders/qwen_3_8b_fp8mixed.safetensors", role: "text_encoder", size: 8664848742 },
        { repo: "black-forest-labs/FLUX.2-small-decoder", file: "full_encoder_small_decoder.safetensors", role: "vae", size: 249519092 },
      ],
    },
    params: { ...IMAGE_SIZE, steps: int(20, 4, 50), cfg: num(5, 1, 10), negative: text("", "Negative prompt") },
    graph: ({ files, prompt, negative, width, height, steps, cfg, seed, prefix }) => ({
      unet: { class_type: "UNETLoader", inputs: { unet_name: files.model, weight_dtype: "default" } },
      clip: { class_type: "CLIPLoader", inputs: { clip_name: files.text_encoder, type: "flux2", device: "default" } },
      vae: { class_type: "VAELoader", inputs: { vae_name: files.vae } },
      pos: { class_type: "CLIPTextEncode", inputs: { text: prompt, clip: ["clip", 0] } },
      neg: { class_type: "CLIPTextEncode", inputs: { text: negative || "", clip: ["clip", 0] } },
      guider: { class_type: "CFGGuider", inputs: { model: ["unet", 0], positive: ["pos", 0], negative: ["neg", 0], cfg } },
      noise: { class_type: "RandomNoise", inputs: { noise_seed: seed } },
      sampler: { class_type: "KSamplerSelect", inputs: { sampler_name: "euler" } },
      sigmas: { class_type: "Flux2Scheduler", inputs: { steps, width, height } },
      latent: { class_type: "EmptyFlux2LatentImage", inputs: { width, height, batch_size: 1 } },
      sample: { class_type: "SamplerCustomAdvanced", inputs: { noise: ["noise", 0], guider: ["guider", 0], sampler: ["sampler", 0], sigmas: ["sigmas", 0], latent_image: ["latent", 0] } },
      decode: { class_type: "VAEDecode", inputs: { samples: ["sample", 0], vae: ["vae", 0] } },
      save: { class_type: "SaveImage", inputs: { images: ["decode", 0], filename_prefix: prefix } },
    }),
  },
  {
    id: "flux1-schnell",
    kind: "image",
    label: "FLUX.1 schnell",
    blurb: "The classic 4-step FLUX.1, all-in-one FP8 checkpoint. Apache-licensed.",
    files: {
      model: { folder: "checkpoints", match: /^flux1-schnell-fp8.*\.safetensors$/i },
    },
    download: {
      items: [{ repo: "Comfy-Org/flux1-schnell", file: "flux1-schnell-fp8.safetensors", role: "model", size: 17236328572 }],
    },
    params: { ...IMAGE_SIZE, steps: int(4, 1, 12) },
    graph: ({ files, prompt, width, height, steps, seed, prefix }) => ({
      ckpt: { class_type: "CheckpointLoaderSimple", inputs: { ckpt_name: files.model } },
      pos: { class_type: "CLIPTextEncode", inputs: { text: prompt, clip: ["ckpt", 1] } },
      neg: { class_type: "CLIPTextEncode", inputs: { text: "", clip: ["ckpt", 1] } },
      latent: { class_type: "EmptySD3LatentImage", inputs: { width, height, batch_size: 1 } },
      sample: {
        class_type: "KSampler",
        inputs: { model: ["ckpt", 0], positive: ["pos", 0], negative: ["neg", 0], latent_image: ["latent", 0], seed, steps, cfg: 1, sampler_name: "euler", scheduler: "simple", denoise: 1 },
      },
      decode: { class_type: "VAEDecode", inputs: { samples: ["sample", 0], vae: ["ckpt", 2] } },
      save: { class_type: "SaveImage", inputs: { images: ["decode", 0], filename_prefix: prefix } },
    }),
  },
  {
    id: "qwen-image-2.1",
    kind: "image",
    label: "Qwen-Image 2.1",
    blurb: "Strong prompt following and text rendering, native 2K. Heavy: wants 12 GB+ VRAM or a GGUF quant.",
    files: {
      model: { folder: "diffusion_models", match: /^qwen[-_]image[-_]2\.1(?!.*vae).*\.(safetensors|gguf)$/i },
      text_encoder: { folder: "text_encoders", match: /^qwen3vl_8b.*\.safetensors$/i },
      vae: { folder: "vae", match: /^qwen[-_]image[-_]2\.1[-_]vae.*\.safetensors$/i },
    },
    download: {
      items: [
        { repo: "Comfy-Org/Qwen-Image-2.1", file: "diffusion_models/qwen_image_2.1_int8_convrot.safetensors", role: "model", size: 7256783064 },
        { repo: "Comfy-Org/Qwen-Image-2.1", file: "text_encoders/qwen3vl_8b_int8_convrot.safetensors", role: "text_encoder", size: 9350798360 },
        { repo: "Comfy-Org/Qwen-Image-2.1", file: "vae/qwen_image_2.1_vae_bf16.safetensors", role: "vae", size: 675509688 },
      ],
    },
    // .gguf weights need the ComfyUI-GGUF custom node (UnetLoaderGGUF); ComfyUI core only loads safetensors.
    nodesFor: (files) => (files.model.endsWith(".gguf") ? ["UnetLoaderGGUF"] : []),
    params: { ...IMAGE_SIZE, steps: int(25, 4, 50), cfg: num(1, 1, 10), negative: text("", "Negative prompt") },
    graph: ({ files, prompt, negative, width, height, steps, cfg, seed, prefix }) => ({
      unet: files.model.endsWith(".gguf")
        ? { class_type: "UnetLoaderGGUF", inputs: { unet_name: files.model } }
        : { class_type: "UNETLoader", inputs: { unet_name: files.model, weight_dtype: "default" } },
      cache: { class_type: "QwenImage21Cache", inputs: { model: ["unet", 0], device: "auto", dtype: "default" } },
      clip: { class_type: "CLIPLoader", inputs: { clip_name: files.text_encoder, type: "qwen_image", device: "default" } },
      vae: { class_type: "VAELoader", inputs: { vae_name: files.vae } },
      cond: { class_type: "TextEncodeQwenImage21", inputs: { clip: ["clip", 0], prompt, negative_prompt: negative || "", resolution: 1024 } },
      latent: { class_type: "EmptyLatentImage", inputs: { width, height, batch_size: 1 } },
      sample: {
        class_type: "KSampler",
        inputs: { model: ["cache", 0], positive: ["cond", 0], negative: ["cond", 1], latent_image: ["latent", 0], seed, steps, cfg, sampler_name: "euler", scheduler: "simple", denoise: 1 },
      },
      decode: { class_type: "VAEDecode", inputs: { samples: ["sample", 0], vae: ["vae", 0] } },
      save: { class_type: "SaveImage", inputs: { images: ["decode", 0], filename_prefix: prefix } },
    }),
  },
  {
    id: "wan2.2-ti2v-5b",
    kind: "video",
    label: "Wan 2.2 TI2V 5B",
    blurb: "Text-to-video at 720p/24 fps. Slow on small GPUs: start with 2-3 seconds.",
    files: {
      model: { folder: "diffusion_models", match: /^wan2\.2_ti2v_5b.*\.safetensors$/i },
      text_encoder: { folder: "text_encoders", match: /^umt5[-_]xxl.*\.safetensors$/i },
      vae: { folder: "vae", match: /^wan2\.2_vae.*\.safetensors$/i },
    },
    download: {
      items: [
        { repo: "Comfy-Org/Wan_2.2_ComfyUI_Repackaged", file: "split_files/diffusion_models/wan2.2_ti2v_5B_fp16.safetensors", role: "model", size: 9999658848 },
        { repo: "Comfy-Org/Wan_2.2_ComfyUI_Repackaged", file: "split_files/vae/wan2.2_vae.safetensors", role: "vae", size: 1409400960 },
        { repo: "Comfy-Org/Wan_2.1_ComfyUI_repackaged", file: "split_files/text_encoders/umt5_xxl_fp8_e4m3fn_scaled.safetensors", role: "text_encoder", size: 6735906897 },
      ],
    },
    params: {
      start_image: image("Start frame (optional): animates this image"),
      width: int(1280, 320, 1920, 32), height: int(704, 320, 1920, 32),
      seconds: num(2, 1, 10, 0.5), fps: int(24, 8, 30), steps: int(20, 4, 50), cfg: num(5, 1, 10),
      // fp16 weights stage ~9.5 GB in system RAM on top of the 6.4 GB text encoder; on a 32 GB machine without a
      // page file that ran out of memory mid-sample. fp8 (cast at load) halves it with little visible difference.
      weights: pick("fp8", ["fp8", "fp16"]),
      negative: text("overexposed, static, blurry details, subtitles, worst quality, low quality, jpeg artifacts, deformed, extra fingers, poorly drawn hands, poorly drawn face, still frame, cluttered background", "Negative prompt"),
    },
    graph: ({ files, prompt, negative, width, height, seconds, fps, steps, cfg, weights, seed, prefix, startImage }) => ({
      ...(startImage ? { img: { class_type: "LoadImage", inputs: { image: startImage } } } : {}),
      unet: { class_type: "UNETLoader", inputs: { unet_name: files.model, weight_dtype: weights === "fp16" ? "default" : "fp8_e4m3fn" } },
      shift: { class_type: "ModelSamplingSD3", inputs: { model: ["unet", 0], shift: 8 } },
      clip: { class_type: "CLIPLoader", inputs: { clip_name: files.text_encoder, type: "wan", device: "default" } },
      vae: { class_type: "VAELoader", inputs: { vae_name: files.vae } },
      pos: { class_type: "CLIPTextEncode", inputs: { text: prompt, clip: ["clip", 0] } },
      neg: { class_type: "CLIPTextEncode", inputs: { text: negative || "", clip: ["clip", 0] } },
      latent: { class_type: "Wan22ImageToVideoLatent", inputs: { vae: ["vae", 0], width, height, length: frames(seconds, fps), batch_size: 1, ...(startImage ? { start_image: ["img", 0] } : {}) } },
      sample: {
        class_type: "KSampler",
        inputs: { model: ["shift", 0], positive: ["pos", 0], negative: ["neg", 0], latent_image: ["latent", 0], seed, steps, cfg, sampler_name: "uni_pc", scheduler: "simple", denoise: 1 },
      },
      decode: { class_type: "VAEDecode", inputs: { samples: ["sample", 0], vae: ["vae", 0] } },
      video: { class_type: "CreateVideo", inputs: { images: ["decode", 0], fps } },
      save: saveVideo(["video", 0], prefix),
    }),
  },
  {
    id: "ltx-video-2b",
    kind: "video",
    label: "LTX-Video 2B (0.9)",
    blurb: "Small, fast text-to-video. Lower quality than Wan, but runs on modest GPUs.",
    files: {
      model: { folder: "checkpoints", match: /^ltx-?video-2b-v0\.9.*\.safetensors$/i },
      text_encoder: { folder: "text_encoders", match: /^t5xxl.*\.safetensors$/i },
    },
    download: {
      items: [
        { repo: "Lightricks/LTX-Video", file: "ltx-video-2b-v0.9.safetensors", role: "model", size: 9370442108 },
        { repo: "comfyanonymous/flux_text_encoders", file: "t5xxl_fp8_e4m3fn.safetensors", role: "text_encoder", size: 4893934904 },
      ],
    },
    params: {
      start_image: image("Start frame (optional): animates this image"),
      width: int(768, 256, 1280, 32), height: int(512, 256, 1280, 32),
      seconds: num(4, 1, 10, 0.5), fps: int(24, 8, 30), steps: int(30, 4, 60), cfg: num(3, 1, 10),
      negative: text("low quality, worst quality, deformed, distorted, disfigured, motion smear, motion artifacts, fused fingers, bad anatomy, weird hand, ugly", "Negative prompt"),
    },
    graph: ({ files, prompt, negative, width, height, seconds, fps, steps, cfg, seed, prefix, startImage }) => {
      const length = frames(seconds, fps, 8);
      // With a start frame, LTXVImgToVideo encodes it into the first latent frame and supplies the latent.
      const i2v = startImage
        ? {
          img: { class_type: "LoadImage", inputs: { image: startImage } },
          i2v: { class_type: "LTXVImgToVideo", inputs: { positive: ["pos", 0], negative: ["neg", 0], vae: ["ckpt", 2], image: ["img", 0], width, height, length, batch_size: 1, strength: 1 } },
        }
        : { latent: { class_type: "EmptyLTXVLatentVideo", inputs: { width, height, length, batch_size: 1 } } };
      const latent = startImage ? ["i2v", 2] : ["latent", 0];
      return {
        ckpt: { class_type: "CheckpointLoaderSimple", inputs: { ckpt_name: files.model } },
        clip: { class_type: "CLIPLoader", inputs: { clip_name: files.text_encoder, type: "ltxv", device: "default" } },
        pos: { class_type: "CLIPTextEncode", inputs: { text: prompt, clip: ["clip", 0] } },
        neg: { class_type: "CLIPTextEncode", inputs: { text: negative || "", clip: ["clip", 0] } },
        ...i2v,
        cond: { class_type: "LTXVConditioning", inputs: { positive: startImage ? ["i2v", 0] : ["pos", 0], negative: startImage ? ["i2v", 1] : ["neg", 0], frame_rate: fps } },
        sched: { class_type: "LTXVScheduler", inputs: { steps, max_shift: 2.05, base_shift: 0.95, stretch: true, terminal: 0.1, latent } },
        sampler: { class_type: "KSamplerSelect", inputs: { sampler_name: "euler" } },
        sample: {
          class_type: "SamplerCustom",
          inputs: { model: ["ckpt", 0], add_noise: true, noise_seed: seed, cfg, positive: ["cond", 0], negative: ["cond", 1], sampler: ["sampler", 0], sigmas: ["sched", 0], latent_image: latent },
        },
        decode: { class_type: "VAEDecode", inputs: { samples: ["sample", 0], vae: ["ckpt", 2] } },
        video: { class_type: "CreateVideo", inputs: { images: ["decode", 0], fps } },
        save: saveVideo(["video", 0], prefix),
      };
    },
  },
  {
    id: "ace-step-1.5",
    kind: "audio",
    label: "ACE-Step 1.5 (music)",
    blurb: "Songs and instrumentals from a style description, with optional lyrics.",
    files: {
      model: { folder: "checkpoints", match: /^ace_step_1\.5.*\.safetensors$/i },
    },
    download: {
      items: [{ repo: "Comfy-Org/ace_step_1.5_ComfyUI_files", file: "checkpoints/ace_step_1.5_turbo_aio.safetensors", role: "model", size: 10025478736 }],
    },
    promptLabel: "Style / tags",
    params: {
      lyrics: text("[instrumental]", "Lyrics"),
      seconds: num(30, 5, 240, 1), bpm: int(120, 40, 220), steps: int(8, 4, 50),
      keyscale: pick("C major", ["C major", "A minor", "G major", "E minor", "D major", "B minor", "F major", "D minor"]),
    },
    graph: ({ files, prompt, lyrics, seconds, bpm, steps, keyscale, seed, prefix }) => ({
      ckpt: { class_type: "CheckpointLoaderSimple", inputs: { ckpt_name: files.model } },
      shift: { class_type: "ModelSamplingAuraFlow", inputs: { model: ["ckpt", 0], shift: 3 } },
      pos: {
        class_type: "TextEncodeAceStepAudio1.5",
        inputs: {
          clip: ["ckpt", 1], tags: prompt, lyrics: lyrics || "", seed, bpm, duration: seconds, timesignature: "4",
          language: "en", keyscale, generate_audio_codes: true, cfg_scale: 2, temperature: 0.85, top_p: 0.9, top_k: 0, min_p: 0,
        },
      },
      neg: { class_type: "ConditioningZeroOut", inputs: { conditioning: ["pos", 0] } },
      latent: { class_type: "EmptyAceStep1.5LatentAudio", inputs: { seconds, batch_size: 1 } },
      sample: {
        class_type: "KSampler",
        inputs: { model: ["shift", 0], positive: ["pos", 0], negative: ["neg", 0], latent_image: ["latent", 0], seed, steps, cfg: 1, sampler_name: "euler", scheduler: "simple", denoise: 1 },
      },
      decode: { class_type: "VAEDecodeAudio", inputs: { samples: ["sample", 0], vae: ["ckpt", 2] } },
      save: saveAudio(["decode", 0], prefix),
    }),
  },
  {
    id: "stable-audio-open",
    kind: "audio",
    label: "Stable Audio Open 1.0",
    blurb: "Sound effects, loops and short music clips up to 47 s.",
    files: {
      model: { folder: "checkpoints", match: /^stable-audio-open.*\.safetensors$/i },
      text_encoder: { folder: "text_encoders", match: /^t5[-_]base.*\.safetensors$/i },
    },
    download: {
      items: [
        { repo: "Comfy-Org/stable-audio-open-1.0_repackaged", file: "stable-audio-open-1.0.safetensors", role: "model", size: 4853889016 },
        { repo: "ComfyUI-Wiki/t5-base", file: "t5-base.safetensors", role: "text_encoder", size: 891646390 },
      ],
    },
    params: { seconds: num(10, 1, 47, 0.5), steps: int(50, 10, 100), cfg: num(5, 1, 10), negative: text("", "Negative prompt") },
    graph: ({ files, prompt, negative, seconds, steps, cfg, seed, prefix }) => ({
      ckpt: { class_type: "CheckpointLoaderSimple", inputs: { ckpt_name: files.model } },
      clip: { class_type: "CLIPLoader", inputs: { clip_name: files.text_encoder, type: "stable_audio", device: "default" } },
      pos: { class_type: "CLIPTextEncode", inputs: { text: prompt, clip: ["clip", 0] } },
      neg: { class_type: "CLIPTextEncode", inputs: { text: negative || "", clip: ["clip", 0] } },
      latent: { class_type: "EmptyLatentAudio", inputs: { seconds, batch_size: 1 } },
      sample: {
        class_type: "KSampler",
        inputs: { model: ["ckpt", 0], positive: ["pos", 0], negative: ["neg", 0], latent_image: ["latent", 0], seed, steps, cfg, sampler_name: "dpmpp_3m_sde_gpu", scheduler: "exponential", denoise: 1 },
      },
      decode: { class_type: "VAEDecodeAudio", inputs: { samples: ["sample", 0], vae: ["ckpt", 2] } },
      save: saveAudio(["decode", 0], prefix),
    }),
  },
];

// Video latents need 4n+1 frames (Wan) or 8n+1 (LTX).
function frames(seconds, fps, step = 4) {
  return Math.max(1, Math.round((seconds * fps) / step)) * step + 1;
}

export const KINDS = ["image", "video", "audio"];

/** Fill in defaults and clamp numbers to a family's parameter schema. */
export function normalizeParams(schema, input) {
  const out = {};
  for (const [k, s] of Object.entries(schema)) {
    let v = input[k] ?? s.default;
    if (s.type === "int" || s.type === "float") {
      v = Number(v);
      if (!Number.isFinite(v)) v = s.default;
      v = Math.min(s.max, Math.max(s.min, v));
      if (s.type === "int") v = Math.round(v / s.step) * s.step;
    } else if (s.type === "select") {
      if (!s.options.includes(v)) v = s.default;
    } else if (s.type === "image") {
      // Only references produced by jobs.mjs (files under outputs/) get this far; anything else is dropped.
      v = typeof v === "string" && /^[\w./-]+\.(png|jpe?g|webp)$/i.test(v) && !v.includes("..") ? v : "";
    } else v = String(v ?? "");
    out[k] = v;
  }
  return out;
}

export const HF = "https://huggingface.co";

/** Download list with direct Hugging Face links and sizes. `roles` limits it to those roles (the missing ones). */
export function downloadInfo(f, roles) {
  const items = f.download.items
    .filter((it) => !roles || roles.includes(it.role))
    .map((it) => ({ ...it, url: `${HF}/${it.repo}/blob/main/${it.file}` }));
  const repos = [...new Set(items.map((it) => it.repo))].map((r) => ({ repo: r, url: `${HF}/${r}` }));
  return {
    gated: f.download.gated || null, items, repos,
    size: items.reduce((s, it) => s + (it.size || 0), 0),
    // A gated file's size can't be read before its license is accepted, so the total is a lower bound then.
    sizeUnknown: items.some((it) => it.size == null),
  };
}

/** Public view of a family (no regexes or functions). */
export function familyInfo(f) {
  return {
    id: f.id, kind: f.kind, label: f.label, blurb: f.blurb, promptLabel: f.promptLabel || "Prompt",
    params: f.params,
    files: Object.fromEntries(Object.entries(f.files).map(([role, r]) => [role, { folder: r.folder }])),
    download: downloadInfo(f),
  };
}
