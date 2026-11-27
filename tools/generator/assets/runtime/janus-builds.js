// Janus-Pro web ONNX builds: the per-module dtypes and devices each variant
// loads. Shared by model-worker.js (what it loads) and custom-model.js (which
// files a custom repo must carry for each variant), so the check and the load
// can't disagree. Published next to both.

// The shader-f16 set is Hugging Face's janus-pro-webgpu reference. Its
// full-precision set (fp32 embeddings, lm_head and gen_head: about 4 GB) does
// not load: ONNX Runtime stages every session's weights through its 4 GB
// wasm32 heap, even for WebGPU sessions, and the concurrent loads overrun it
// ("memory access out of bounds" — seen on an RTX 4070 Ti under Chrome on
// Linux, which exposes no shader-f16). So without shader-f16 the embeddings
// are 8-bit and the heads 4-bit weights (fp32 compute), about 1.6 GB in all.
export const JANUS_DTYPES = Object.freeze({
  f16: Object.freeze({
    prepare_inputs_embeds: "q4",
    language_model: "q4f16",
    lm_head: "fp16",
    gen_head: "fp16",
    gen_img_embeds: "fp16",
    image_decode: "fp32",
  }),
  f32: Object.freeze({
    prepare_inputs_embeds: "q8",
    language_model: "q4",
    lm_head: "q4",
    gen_head: "q4",
    gen_img_embeds: "fp32",
    image_decode: "fp32",
  }),
});

// Whether each variant's dtypes need the adapter's shader-f16 feature.
export const JANUS_NEEDS_F16 = Object.freeze({ f16: true, f32: false });

// Input embedding preparation runs on WebAssembly (an upstream WebGPU issue);
// every generation stage runs on WebGPU.
export const JANUS_DEVICES = Object.freeze({
  prepare_inputs_embeds: "wasm",
  language_model: "webgpu",
  lm_head: "webgpu",
  gen_head: "webgpu",
  gen_img_embeds: "webgpu",
  image_decode: "webgpu",
});

const hasF16 = (adapter) => Boolean(adapter && adapter.features && adapter.features.has("shader-f16"));

// The variant to load for `model` on `adapter`. A descriptor's `variant` is
// the build the user accepted (and whose files were checked), so it is loaded
// as named; only the legacy imagine descriptor, which names none, is picked
// from the adapter's shader-f16 support.
export function janusVariant(model, adapter) {
  const variant = janusBuild(model, adapter);
  if (!Object.hasOwn(JANUS_DTYPES, variant)) throw new Error(`The image model has no "${variant}" build.`);
  if (JANUS_NEEDS_F16[variant] && !hasF16(adapter)) {
    throw new Error(`The "${variant}" build needs a GPU with shader-f16, which this device doesn't provide.`);
  }
  return variant;
}

// The build `model` loads on `adapter`, unchecked: the descriptor's `variant`
// as named, or for the legacy descriptor the one its adapter picks.
export function janusBuild(model, adapter) {
  const variant = model && model.variant;
  if (variant === undefined || variant === null || variant === "") return hasF16(adapter) ? "f16" : "f32";
  return variant;
}

// One resident Janus load per model build: a descriptor without a variant
// shares the load of the one naming the build it would pick.
export function janusLoadKey(model, adapter) {
  return `${model.id}@${model.revision}|${janusBuild(model, adapter)}`;
}
