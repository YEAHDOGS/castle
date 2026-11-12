// ComfyUI API-format graphs for FLUX.3 Image Klein (distilled int8), derived from the official
// Comfy-Org templates image_flux3_klein_distilled_t2i / _image_edit with the 9B prompt upsampler removed
// (the opencode agent writes the detailed caption itself, and the upsampler does not fit next to it in 6 GB).

export const MODEL_FILES = {
  diffusion_models: "flux3-image-klein-int8-convrot.safetensors",
  text_encoders: "qwen3vl_4b_int8_convrot.safetensors",
  vae: "flux2-vae.safetensors",
};

const SAMPLER = { cfg: 1, sampler_name: "euler", scheduler: "simple", denoise: 1 };

function loaders() {
  return {
    unet: { class_type: "UNETLoader", inputs: { unet_name: MODEL_FILES.diffusion_models, weight_dtype: "default" } },
    clip: { class_type: "CLIPLoader", inputs: { clip_name: MODEL_FILES.text_encoders, type: "flux3", device: "default" } },
    vae: { class_type: "VAELoader", inputs: { vae_name: MODEL_FILES.vae } },
  };
}

export function textToImage({ prompt, width, height, steps, count, seed, filename_prefix }) {
  return {
    ...loaders(),
    pos: { class_type: "CLIPTextEncode", inputs: { text: prompt, clip: ["clip", 0] } },
    neg: { class_type: "CLIPTextEncode", inputs: { text: "", clip: ["clip", 0] } },
    latent: { class_type: "EmptyFlux2LatentImage", inputs: { width, height, batch_size: count } },
    sample: {
      class_type: "KSampler",
      inputs: { model: ["unet", 0], positive: ["pos", 0], negative: ["neg", 0], latent_image: ["latent", 0], seed, steps, ...SAMPLER },
    },
    decode: { class_type: "VAEDecode", inputs: { samples: ["sample", 0], vae: ["vae", 0] } },
    save: { class_type: "SaveImage", inputs: { images: ["decode", 0], filename_prefix } },
  };
}

// images: names already uploaded to ComfyUI's input folder, in <ref_image_N> order.
export function imageEdit({ prompt, images, width, height, steps, seed, filename_prefix }) {
  const g = {
    ...loaders(),
    pos: { class_type: "CLIPTextEncode", inputs: { text: prompt, clip: ["clip", 0] } },
    neg: { class_type: "ConditioningZeroOut", inputs: { conditioning: ["pos", 0] } },
    refs: {
      class_type: "ReferenceMultiLatent",
      // V3 dynamic inputs: DynamicCombo children are "<id>.<child>", Autogrow slots are "<id>.<prefix><n>".
      inputs: { positive: ["pos", 0], negative: ["neg", 0], vae: ["vae", 0], resolution: "resize", "resolution.megapixels": 1 },
    },
  };
  images.forEach((name, i) => {
    g[`img${i}`] = { class_type: "LoadImage", inputs: { image: name } };
    g.refs.inputs[`images.image${i}`] = [`img${i}`, 0];
  });
  let latent = ["refs", 2];
  if (width && height) {
    g.latent = { class_type: "EmptyFlux2LatentImage", inputs: { width, height, batch_size: 1 } };
    latent = ["latent", 0];
  }
  g.sample = {
    class_type: "KSampler",
    inputs: { model: ["unet", 0], positive: ["refs", 0], negative: ["refs", 1], latent_image: latent, seed, steps, ...SAMPLER },
  };
  g.decode = { class_type: "VAEDecode", inputs: { samples: ["sample", 0], vae: ["vae", 0] } };
  g.save = { class_type: "SaveImage", inputs: { images: ["decode", 0], filename_prefix } };
  return g;
}
