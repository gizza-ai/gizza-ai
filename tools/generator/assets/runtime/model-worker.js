// Runs Transformers.js pipelines away from the page's main thread. The
// runtime itself is self-hosted; pinned model weights are fetched lazily and
// then served from Transformers.js' browser cache.

import {
  AutoModel,
  AutoProcessor,
  BaseStreamer,
  env,
  ImageProcessor,
  InterruptableStoppingCriteria,
  MultiModalityCausalLM,
  pipeline,
  RawImage,
} from "./transformers.min.js";
import { downloadTracker } from "./model-download-progress.js";
import { JANUS_DEVICES, JANUS_DTYPES, janusLoadKey, janusVariant } from "./janus-builds.js";

env.backends.onnx.wasm.wasmPaths = new URL("./", import.meta.url).href;
env.backends.onnx.wasm.proxy = false;
env.allowLocalModels = false;
env.allowRemoteModels = true;
env.useBrowserCache = true;

const loadedRunners = new Map();

const U2NET_PROCESSOR_CONFIG = {
  do_convert_rgb: true,
  do_normalize: true,
  do_pad: true,
  do_rescale: true,
  do_resize: true,
  keep_aspect_ratio: true,
  image_mean: [0.485, 0.456, 0.406],
  image_std: [0.229, 0.224, 0.225],
  pad_size: { width: 320, height: 320 },
  size: { longest_edge: 320 },
};

// A WebAssembly trap (ONNX Runtime running out of its wasm heap, typically)
// leaves the runtime's memory unusable: the client must replace this worker.
function isFatal(error) {
  return typeof WebAssembly !== "undefined" && error instanceof WebAssembly.RuntimeError;
}

function errorMessage(error) {
  if (isFatal(error)) {
    return "The AI model ran out of memory on this device. Try again; if it keeps failing, this device can't run it.";
  }
  if (typeof error === "string") return error;
  if (error && typeof error.message === "string") return error.message;
  return "Unknown inference error";
}

function pipelineKey(model, device, dtype) {
  return [model.task, model.id, model.revision, device, dtype || "auto"].join("|");
}

async function assertDeviceAvailable(device) {
  if (device !== "webgpu") return;
  const gpu = self.navigator && self.navigator.gpu;
  if (!gpu || typeof gpu.requestAdapter !== "function") {
    throw new Error("WebGPU is not available in this browser");
  }
  const adapter = await gpu.requestAdapter();
  if (!adapter) {
    throw new Error("WebGPU is present but no usable GPU adapter is available");
  }
}

async function getRunner(id, model, device) {
  const dtype = model.dtypes && model.dtypes[device] ? model.dtypes[device] : undefined;
  const key = pipelineKey(model, device, dtype);
  let promise = loadedRunners.get(key);
  if (!promise) {
    const options = {
      revision: model.revision,
      device,
      progress_callback(progress) {
        postMessage({
          type: "progress",
          id,
          progress: { ...progress, stage: "download", device },
        });
      },
    };
    if (dtype) options.dtype = dtype;
    if (model.task === "u2net-background-removal") {
      promise = AutoModel.from_pretrained(model.id, options).then((loadedModel) => ({
        type: "u2net",
        model: loadedModel,
        processor: new ImageProcessor(U2NET_PROCESSOR_CONFIG),
      }));
    } else {
      promise = pipeline(model.task, model.id, options);
    }
    loadedRunners.set(key, promise);
    promise.catch(() => loadedRunners.delete(key));
  }
  return promise;
}

function alphaMetrics(image) {
  if (!image || image.channels !== 4 || !image.data) return null;
  const pixelCount = image.width * image.height;
  let alphaSum = 0;
  let foregroundPixels = 0;
  let backgroundPixels = 0;
  let opaquePixels = 0;
  let transparentPixels = 0;
  for (let index = 3; index < image.data.length; index += 4) {
    const alpha = image.data[index];
    alphaSum += alpha;
    if (alpha >= 16) foregroundPixels += 1;
    if (alpha <= 239) backgroundPixels += 1;
    if (alpha >= 239) opaquePixels += 1;
    if (alpha <= 16) transparentPixels += 1;
  }
  return {
    meanAlpha: alphaSum / pixelCount / 255,
    foregroundRatio: foregroundPixels / pixelCount,
    backgroundRatio: backgroundPixels / pixelCount,
    opaqueRatio: opaquePixels / pixelCount,
    transparentRatio: transparentPixels / pixelCount,
  };
}

async function runBackgroundRemoval(pipe, file) {
  const output = await pipe(file);
  const image = Array.isArray(output) ? output[0] : output;
  if (!image || typeof image.toBlob !== "function") {
    throw new Error("The background-removal model returned no image.");
  }
  return {
    blob: await image.toBlob("image/png"),
    metrics: alphaMetrics(image),
  };
}

async function runU2NetBackgroundRemoval(runner, file) {
  const image = await RawImage.fromBlob(file);
  const inputs = await runner.processor(image);
  const session = runner.model.sessions.model;
  const inputName = session.inputNames[0];
  const outputName = runner.model.config.output_composite || session.outputNames[0];
  const output = await runner.model({ [inputName]: inputs.pixel_values });
  const matte = output[outputName][0];
  const epsilon = 1e-5;
  if (matte.data.some((value) => value < -epsilon || value > 1 + epsilon)) {
    matte.sigmoid_();
  }

  let mask = await RawImage.fromTensor(matte.mul_(255).to("uint8"));
  const [reshapedHeight, reshapedWidth] = inputs.reshaped_input_sizes[0];
  if (mask.width !== reshapedWidth || mask.height !== reshapedHeight) {
    mask = await mask.crop([0, 0, reshapedWidth - 1, reshapedHeight - 1]);
  }
  mask = await mask.resize(image.width, image.height);
  const result = image.clone();
  result.putAlpha(mask);
  return {
    blob: await result.toBlob("image/png"),
    metrics: alphaMetrics(result),
  };
}

async function runTask(pipe, task, file) {
  switch (task) {
    case "background-removal":
      return runBackgroundRemoval(pipe, file);
    case "u2net-background-removal":
      return runU2NetBackgroundRemoval(pipe, file);
    default:
      throw new Error(`Unsupported browser model task: ${task}`);
  }
}

// ---- Text-to-image (Janus-Pro via MultiModalityCausalLM) --------------------
// Request:  { type: "generate", id, model, prompt }   (model.task "text-to-image")
//           { type: "cancel", id }
// Replies:  progress { stage: "download", progress: 0..1 } while weights load,
//           progress { stage: "generate", count, total } per image token,
//           then { type: "result", id, blob, filename, backend } |
//           { type: "cancelled", id } | { type: "error", id, error, fatal }
//           (fatal: the worker's runtime is broken; replace the worker).
//           progress { stage: "loading" } is posted only while the weights
//           are not yet loaded. Transformers.js can't abort a download, so
//           the client terminates this worker on a cancel in that window;
//           here a cancel while loading just stops this generation waiting.
//           The descriptor's `variant` (f16 | f32) picks the build; a
//           descriptor without one (the imagine page) is sniffed from the GPU.

const WEBGPU_REQUIRED = "Image generation needs WebGPU, which this browser or device doesn't provide.";

// One Janus load per worker, shared by every generation that waits on it.
// The client keeps one model build per worker (it replaces the worker to
// switch), so a load for another key only follows a failed one. The key is
// the build the load picks on this adapter, so the imagine page's
// variant-less descriptor shares the load of the registry's descriptor for
// the same build. Download progress goes to every generation currently
// waiting on the load.
let janus = null; // { key, promise, ready, waiters: Set<generation id> }

async function gpuAdapter() {
  const gpu = self.navigator && self.navigator.gpu;
  const adapter = gpu && typeof gpu.requestAdapter === "function" ? await gpu.requestAdapter() : null;
  if (!adapter) throw new Error(WEBGPU_REQUIRED);
  return adapter;
}

// Synchronous from the adapter on, so concurrent generations see each
// other's load.
function loadJanus(model, adapter) {
  const key = janusLoadKey(model, adapter);
  if (janus && janus.key === key) return janus;
  const waiters = new Set();
  const progress_callback = downloadTracker((fraction, loaded, total) => {
    for (const id of waiters) {
      postMessage({ type: "progress", id, progress: { stage: "download", progress: fraction, loaded, total } });
    }
  }, (model.download_mb || 0) * 1e6);
  const promise = (async () => {
    const variant = janusVariant(model, adapter);
    return Promise.all([
      AutoProcessor.from_pretrained(model.id, { revision: model.revision, progress_callback }),
      MultiModalityCausalLM.from_pretrained(model.id, {
        revision: model.revision,
        dtype: JANUS_DTYPES[variant],
        device: JANUS_DEVICES,
        progress_callback,
      }),
    ]);
  })();
  const load = { key, promise, ready: false, waiters };
  janus = load;
  promise.then(
    () => {
      load.ready = true;
    },
    () => {
      if (janus === load) janus = null;
    },
  );
  return load;
}

// Counts generated image tokens (the first put() is the prompt).
class TokenProgress extends BaseStreamer {
  constructor(total, onToken) {
    super();
    this.total = total;
    this.onToken = onToken;
    this.count = null;
  }
  put() {
    if (this.count === null) {
      this.count = 0;
      return;
    }
    this.count += 1;
    this.onToken(this.count, this.total);
  }
  end() {}
}

const CANCELLED = Symbol("cancelled");
// id → { stopping: InterruptableStoppingCriteria, cancelled: Promise, cancel() }
const generations = new Map();

function startGeneration(id) {
  let cancel;
  const cancelled = new Promise((resolve) => {
    cancel = () => resolve(CANCELLED);
  });
  const generation = { stopping: new InterruptableStoppingCriteria(), cancelled, cancel };
  generations.set(id, generation);
  return generation;
}

// Waits for the shared model load, or for this generation's cancel —
// whichever comes first. Returns CANCELLED on cancel.
async function awaitJanus(id, load, generation) {
  load.waiters.add(id);
  try {
    return await Promise.race([load.promise, generation.cancelled]);
  } finally {
    load.waiters.delete(id);
  }
}

async function generateImage(request) {
  const { id, model, prompt } = request;
  const generation = startGeneration(id);
  const { stopping } = generation;
  try {
    if (!model || model.task !== "text-to-image") {
      throw new Error(`Unsupported browser model task: ${model && model.task}`);
    }
    const text = String(prompt || "").trim();
    if (!text) throw new Error("Describe the image you want first.");
    const adapter = await gpuAdapter();
    if (stopping.interrupted) {
      postMessage({ type: "cancelled", id });
      return;
    }
    const load = loadJanus(model, adapter);
    if (!load.ready) postMessage({ type: "progress", id, progress: { stage: "loading", device: "webgpu" } });
    const loaded = await awaitJanus(id, load, generation);
    if (loaded === CANCELLED || stopping.interrupted) {
      postMessage({ type: "cancelled", id });
      return;
    }
    const [processor, janusModel] = loaded;
    const total = processor.num_image_tokens;
    // The weights are in: from here a cancel stops the run, not the worker.
    postMessage({ type: "progress", id, progress: { stage: "generate", count: 0, total } });
    const inputs = await processor([{ role: "<|User|>", content: text }], { chat_template: "text_to_image" });
    const streamer = new TokenProgress(total, (count) => {
      postMessage({ type: "progress", id, progress: { stage: "generate", count, total } });
    });
    let outputs;
    try {
      outputs = await janusModel.generate_images({
        ...inputs,
        min_new_tokens: total,
        max_new_tokens: total,
        do_sample: true,
        streamer,
        stopping_criteria: stopping,
      });
    } catch (error) {
      // An interrupted run stops short of a full token grid; decoding it fails.
      if (stopping.interrupted) {
        postMessage({ type: "cancelled", id });
        return;
      }
      throw error;
    }
    if (stopping.interrupted) {
      postMessage({ type: "cancelled", id });
      return;
    }
    const blob = await outputs[0].toBlob("image/png");
    postMessage({ type: "result", id, blob, filename: "generated.png", backend: "webgpu" });
  } catch (error) {
    postMessage({ type: "error", id, error: errorMessage(error), fatal: isFatal(error) });
  } finally {
    generations.delete(id);
  }
}

self.addEventListener("message", (event) => {
  const request = event.data || {};
  if (request.type === "generate") generateImage(request);
  else if (request.type === "cancel") {
    // While the model loads, cancel answers at once (awaitJanus stops
    // waiting; the client also terminates the worker to stop the download);
    // while generating, the stopping criteria end the run at the next image
    // token.
    const generation = generations.get(request.id);
    if (generation) {
      generation.stopping.interrupt();
      generation.cancel();
    }
  }
});

self.addEventListener("message", async (event) => {
  const request = event.data || {};
  if (request.type !== "run") return;
  const { id, model, file } = request;
  const devices = Array.isArray(model && model.devices) ? model.devices : [];
  const failures = [];

  for (let index = 0; index < devices.length; index += 1) {
    const device = devices[index];
    try {
      postMessage({
        type: "progress",
        id,
        progress: { stage: index ? "fallback" : "loading", device },
      });
      // Some browsers expose navigator.gpu even when requestAdapter() cannot
      // return a device (notably headless Chromium and disabled/blocked GPUs).
      // Probe before initializing ONNX: a failed WebGPU session can otherwise
      // poison backend selection and prevent the WASM retry from starting.
      await assertDeviceAvailable(device);
      const pipe = await getRunner(id, model, device);
      postMessage({ type: "progress", id, progress: { stage: "inference", device } });
      const result = await runTask(pipe, model.task, file);
      postMessage({
        type: "result",
        id,
        blob: result.blob,
        metrics: result.metrics,
        filename: "background-removed.png",
        backend: device,
      });
      return;
    } catch (error) {
      failures.push(`${device}: ${errorMessage(error)}`);
    }
  }

  postMessage({
    type: "error",
    id,
    error: failures.length
      ? `AI inference failed (${failures.join("; ")})`
      : "No compatible AI inference backend is available.",
  });
});
