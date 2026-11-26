// Shared client for standalone AI-model tool pages. Heavy inference runs in a
// module worker so model loading and CPU/WASM fallback do not freeze the UI.

const MODEL_WORKER_URL = new URL("../_model-runtime/model-worker.js", import.meta.url);

export function compatibleDevices(model, nav = globalThis.navigator || {}) {
  const requested = Array.isArray(model && model.devices) && model.devices.length
    ? model.devices
    : ["wasm"];
  return requested.filter((device, index) => {
    if (requested.indexOf(device) !== index) return false;
    if (device === "webgpu") return Boolean(nav.gpu);
    return device === "wasm";
  });
}

export function progressLabel(progress) {
  if (!progress || typeof progress !== "object") return "Loading AI model…";
  if (progress.stage === "inference") {
    return `Processing locally with ${progress.device || "AI"}…`;
  }
  if (progress.stage === "fallback") {
    return `Trying ${progress.device || "the CPU fallback"}…`;
  }
  if (progress.stage === "generate") {
    const total = Number(progress.total);
    const percent = total > 0 ? Math.round((Number(progress.count) / total) * 100) : 0;
    return `Generating image… ${Math.max(0, Math.min(100, percent))}%`;
  }
  const value = Number(progress.progress);
  if (Number.isFinite(value)) {
    const percent = value <= 1 ? value * 100 : value;
    return `Downloading AI model… ${Math.max(0, Math.min(100, Math.round(percent)))}%`;
  }
  return "Loading AI model…";
}

// What an image-input model tool accepts. Exported so a page that prepares a
// model input itself (e.g. an editor whose working copy can be larger) fits it
// to the same limits validateModelFile enforces.
export const MODEL_FILE_LIMITS = Object.freeze({
  maxBytes: 20 * 1024 * 1024,
  maxPixels: 40_000_000,
  types: Object.freeze(["image/png", "image/jpeg", "image/webp"]),
});

export async function validateModelFile(file, options = {}) {
  const maxBytes = options.maxBytes || MODEL_FILE_LIMITS.maxBytes;
  const maxPixels = options.maxPixels || MODEL_FILE_LIMITS.maxPixels;
  const supportedTypes = new Set(MODEL_FILE_LIMITS.types);
  if (!file) throw new Error("Choose an image first.");
  if (!supportedTypes.has(String(file.type || "").toLowerCase())) {
    throw new Error("Choose a PNG, JPEG, or WebP image.");
  }
  if (file.size > maxBytes) {
    throw new Error(`Image is too large; the limit is ${Math.round(maxBytes / 1024 / 1024)} MB.`);
  }
  if (typeof createImageBitmap === "function") {
    const bitmap = await createImageBitmap(file);
    const pixels = bitmap.width * bitmap.height;
    const dimensions = { width: bitmap.width, height: bitmap.height };
    bitmap.close();
    if (!dimensions.width || !dimensions.height) throw new Error("The image has no pixels.");
    if (pixels > maxPixels) {
      throw new Error(`Image is too large; the limit is ${Math.round(maxPixels / 1_000_000)} megapixels.`);
    }
    return dimensions;
  }
  return null;
}

export function validateModelResult(result, options = {}) {
  if (!result || !(result.blob instanceof Blob)) {
    throw new Error("The AI model returned no image.");
  }
  const minimumCoverage = options.minimumCoverage || 0.005;
  const minimumSolidCoverage = options.minimumSolidCoverage || 0.001;
  const metrics = result.metrics;
  if (metrics && Number(metrics.foregroundRatio) < minimumCoverage) {
    throw new Error(
      "No foreground subject was detected. Try an image with one clear main subject.",
    );
  }
  if (metrics && Number(metrics.backgroundRatio) < minimumCoverage) {
    throw new Error(
      "The subject could not be separated from its background. Try an image with more contrast.",
    );
  }
  if (metrics && Number.isFinite(Number(metrics.opaqueRatio))
      && Number(metrics.opaqueRatio) < minimumSolidCoverage) {
    throw new Error(
      "The foreground mask is too faint. Try an image with a clearer, better-lit subject.",
    );
  }
  if (metrics && Number.isFinite(Number(metrics.transparentRatio))
      && Number(metrics.transparentRatio) < minimumSolidCoverage) {
    throw new Error(
      "The background was not made transparent. Try an image with more contrast.",
    );
  }
  return result;
}

export class ModelWorkerClient {
  constructor(workerFactory = () => new Worker(MODEL_WORKER_URL, { type: "module" })) {
    this.worker = workerFactory();
    this.nextId = 1;
    this.pending = new Map();
    // A stopped client (worker crashed or disposed) never runs again; the
    // shared accessor below replaces it with a fresh worker.
    this.stopped = false;
    this.worker.addEventListener("message", (event) => this.onMessage(event.data));
    this.worker.addEventListener("error", (event) => {
      this.stop(event && event.message ? event.message : "AI worker failed.");
    });
  }

  // Terminate the worker and fail every pending call, detaching their abort
  // listeners so a later abort cannot post to the dead worker.
  stop(message) {
    if (this.stopped) return;
    this.stopped = true;
    this.worker.terminate();
    const pending = [...this.pending.values()];
    this.pending.clear();
    for (const { reject, cleanup } of pending) {
      if (cleanup) cleanup();
      reject(new Error(message));
    }
  }

  onMessage(message) {
    const pending = this.pending.get(message && message.id);
    if (!pending) return;
    if (message.type === "progress") {
      if (pending.onProgress) pending.onProgress(message.progress);
      return;
    }
    this.pending.delete(message.id);
    if (pending.cleanup) pending.cleanup();
    if (message.type === "result") pending.resolve(message);
    else if (message.type === "cancelled") pending.reject(abortError());
    else pending.reject(new Error(message.error || "AI inference failed."));
  }

  // Text-to-image: `prompt` → { blob, filename, backend }. Aborting `signal`
  // asks the worker to stop at the next image token and rejects with an
  // AbortError once it has.
  generate(model, prompt, { onProgress = null, signal = null } = {}) {
    if (signal && signal.aborted) return Promise.reject(abortError());
    const id = this.nextId++;
    return new Promise((resolve, reject) => {
      const onAbort = () => this.worker.postMessage({ type: "cancel", id });
      const cleanup = signal ? () => signal.removeEventListener("abort", onAbort) : null;
      // A result the worker posted before it saw the cancel still counts as
      // cancelled: the caller has already moved on.
      const settle = (message) => (signal && signal.aborted ? reject(abortError()) : resolve(message));
      this.pending.set(id, { resolve: settle, reject, onProgress, cleanup });
      if (signal) signal.addEventListener("abort", onAbort, { once: true });
      this.worker.postMessage({ type: "generate", id, model, prompt });
    });
  }

  run(model, file, fields = {}, onProgress = null) {
    const id = this.nextId++;
    return new Promise((resolve, reject) => {
      this.pending.set(id, { resolve, reject, onProgress });
      this.worker.postMessage({
        type: "run",
        id,
        model: { ...model, devices: compatibleDevices(model) },
        file,
        fields,
      });
    });
  }

  dispose() {
    this.stop("AI worker stopped.");
  }
}

function abortError() {
  return new DOMException("Image generation was cancelled.", "AbortError");
}

let sharedClient = null;

function client() {
  if (!sharedClient || sharedClient.stopped) sharedClient = new ModelWorkerClient();
  return sharedClient;
}

// Generate an image locally from `prompt` with a pinned text-to-image model
// (`model.task === "text-to-image"`). WebGPU only.
export function generateImage(model, prompt, { onProgress = null, signal = null, nav = globalThis.navigator || {} } = {}) {
  if (!nav.gpu) {
    return Promise.reject(new Error("Image generation needs WebGPU, which this browser or device doesn't provide."));
  }
  if (!model || !model.id || !model.revision) {
    return Promise.reject(new Error("This AI tool is missing its pinned model configuration."));
  }
  if (model.task !== "text-to-image") {
    return Promise.reject(new Error(`Expected a text-to-image model, got "${model.task}".`));
  }
  if (!String(prompt || "").trim()) {
    return Promise.reject(new Error("Describe the image you want first."));
  }
  return client().generate(model, String(prompt).trim(), { onProgress, signal });
}

export function runModel(model, file, fields, onProgress) {
  if (!model || !model.task || !model.id || !model.revision) {
    return Promise.reject(new Error("This AI tool is missing its pinned model configuration."));
  }
  if (!compatibleDevices(model).length) {
    return Promise.reject(new Error("This browser supports neither WebGPU nor the WebAssembly fallback."));
  }
  return client().run(model, file, fields, onProgress);
}
