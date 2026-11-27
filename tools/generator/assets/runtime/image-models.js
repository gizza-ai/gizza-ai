// Curated image-model registry for the image-model picker. Entries have the
// same shape custom-model.js returns for custom repos, so the picker treats
// both alike. A variant is only ever chosen when `verified` is true; flip
// `fp16.verified` for SD-Turbo once it has been proven on a real f16 GPU.

export const IMAGE_MODELS = Object.freeze([
  {
    id: "sd-turbo",
    label: "SD-Turbo",
    tagline: "Fast 512x512 images in a few steps",
    engine: "sd-turbo",
    repo: "gizza-ai/sd-turbo-web",
    revision: "d2df2e40f2251622ec60053b0e9bfa8c64b7f14d",
    output: { w: 512, h: 512 },
    license: {
      name: "Stability AI Community License",
      url: "https://huggingface.co/gizza-ai/sd-turbo-web/blob/d2df2e40f2251622ec60053b0e9bfa8c64b7f14d/LICENSE.md",
      summary:
        "Free, including commercial use under $1M annual revenue; the Acceptable Use Policy applies.",
      attribution: "Powered by Stability AI",
    },
    recommended: true,
    variants: {
      nof16q8: { requires: { shaderF16: false }, download_mb: 1735, verified: true },
      fp16: { requires: { shaderF16: true }, download_mb: 2517, verified: false },
    },
  },
  {
    id: "janus-pro-1b",
    label: "Janus-Pro 1B",
    tagline: "DeepSeek's multimodal model, 384x384 images",
    engine: "janus",
    repo: "onnx-community/Janus-Pro-1B-ONNX",
    revision: "04efdf2e36cb07a034b0d94f7322356b292f0418",
    output: { w: 384, h: 384 },
    license: {
      name: "DeepSeek Model License",
      url: "https://github.com/deepseek-ai/DeepSeek-LLM/blob/HEAD/LICENSE-MODEL",
      summary: "Open weights with use-based restrictions; commercial use is allowed.",
    },
    // f16 matches the imagine tool's page model (blocks/imagine/page/meta.toml).
    variants: {
      f16: { requires: { shaderF16: true }, download_mb: 2410, verified: true },
      f32: { requires: { shaderF16: false }, download_mb: 1645, verified: true },
    },
  },
]);

// Largest single weight buffer each engine allocates (bytes), below which the
// adapter is likely to fail the load.
const MIN_BUFFER = { "sd-turbo": 2 ** 30, janus: 2 ** 29 };

export async function adapterInfo(nav) {
  const none = { webgpu: false, shaderF16: false, maxBufferSize: 0 };
  if (!nav?.gpu) return none;
  let adapter = null;
  try {
    adapter = await nav.gpu.requestAdapter();
  } catch {
    return none;
  }
  if (!adapter) return none;
  return {
    webgpu: true,
    shaderF16: adapter.features?.has("shader-f16") === true,
    maxBufferSize: Number(adapter.limits?.maxBufferSize) || 0,
  };
}

// Best verified variant this adapter can run; f16 variants win when usable.
export function resolveVariant(entry, info) {
  if (!info?.webgpu) return null;
  const usable = Object.entries(entry.variants)
    .filter(([, v]) => v.verified && (!v.requires.shaderF16 || info.shaderF16))
    .sort(([, a], [, b]) => Number(b.requires.shaderF16) - Number(a.requires.shaderF16));
  if (usable.length === 0) return null;
  const [name, v] = usable[0];
  return {
    name,
    descriptor: {
      task: "text-to-image",
      engine: entry.engine,
      id: entry.repo,
      revision: entry.revision,
      variant: name,
      download_mb: v.download_mb,
      output: entry.output,
    },
  };
}

export function deviceFit(entry, info) {
  if (!info?.webgpu) {
    return { level: "unsupported", note: "This browser has no WebGPU, which image models need." };
  }
  if (!resolveVariant(entry, info)) {
    return { level: "unsupported", note: "No verified build of this model fits this GPU." };
  }
  const need = MIN_BUFFER[entry.engine] ?? 0;
  if (info.maxBufferSize < need) {
    return { level: "risky", note: "This GPU's buffer limit is low; loading may fail." };
  }
  return { level: "ok", note: "Should run on this device." };
}

// First recommended entry that fits this device, else any entry that fits,
// else the first entry that is not unsupported, else the recommended entry (so the picker always has one).
export function defaultModel(info) {
  const fit = (m) => deviceFit(m, info).level;
  const recommended = IMAGE_MODELS.filter((m) => m.recommended);
  return (
    recommended.find((m) => fit(m) === "ok") ??
    IMAGE_MODELS.find((m) => fit(m) === "ok") ??
    IMAGE_MODELS.find((m) => fit(m) !== "unsupported") ??
    recommended[0] ??
    IMAGE_MODELS[0]
  );
}
