// Curated image-model registry for the image-model picker. Entries have the
// same shape custom-model.js returns for custom repos, so the picker treats
// both alike. `license` is { name, url, summary, attribution?, aup? }: `aup`
// links the Acceptable Use Policy a summary refers to. Verified variants are preferred; flip
// `fp16.verified` for SD-Turbo once it has been proven on a real f16 GPU.

import { generateOn } from "./tool-model.js";

// The terms every SD-Turbo-derived model carries, curated or custom: Stability
// AI's Community License asks for this attribution and binds its Acceptable
// Use Policy.
export const STABILITY_TERMS = Object.freeze({
  aup: "https://stability.ai/use-policy",
  attribution: "Powered by Stability AI",
});

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
      ...STABILITY_TERMS,
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

// Whether the curated entries prove the `name` build of `engine` (e.g. SD-Turbo's
// nof16q8, not yet its fp16). A custom repo in an engine's layout is never
// verified itself, but carries this trust: its builds are the same graphs.
function provenBuild(engine, name) {
  return IMAGE_MODELS.some((m) => m.engine === engine && m.variants[name]?.verified === true);
}

// Best variant this adapter can run: verified ones win, then builds the
// curated entries prove for the engine, then f16 ones. An unverified variant
// is used only when no verified one is compatible (custom entries are never
// verified).
export function resolveVariant(entry, info) {
  if (!info?.webgpu) return null;
  const usable = Object.entries(entry.variants)
    .filter(([, v]) => !v.requires.shaderF16 || info.shaderF16)
    .sort(
      ([an, a], [bn, b]) =>
        Number(b.verified) - Number(a.verified) ||
        Number(provenBuild(entry.engine, bn)) - Number(provenBuild(entry.engine, an)) ||
        Number(b.requires.shaderF16) - Number(a.requires.shaderF16),
    );
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
    // Every build needs shader-f16 (the only build requirement), which this GPU lacks.
    return {
      level: "unsupported",
      note: "No build of this model runs on this GPU: it needs shader-f16, which this GPU doesn't provide.",
    };
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

// Generate with a resolved descriptor (resolveVariant(...).descriptor) on its
// engine: `prompt` → { blob, filename, backend }. Same progress, cancel and
// fatal contract as generateImage. Another resident model finishes its
// running calls first (this call reports progress stage "queued" meanwhile);
// then its worker is terminated and this model's starts.
export function generateWith(descriptor, prompt, { onProgress = null, signal = null, nav = globalThis.navigator || {} } = {}) {
  return generateOn(descriptor && descriptor.engine, descriptor, prompt, { onProgress, signal, nav });
}
