// Shared client for standalone AI-model tool pages. Heavy inference runs in a
// module worker so model loading and CPU/WASM fallback do not freeze the UI.

// Each engine's worker. model-worker.js hosts Transformers.js (Janus and the
// image-input models); sd-worker.js hosts SD-Turbo on its own ONNX Runtime.
export const MODEL_WORKER_URL = new URL("../_model-runtime/model-worker.js", import.meta.url);
export const SD_WORKER_URL = new URL("../_model-runtime/sd-worker.js", import.meta.url);

// The worker factory hook: a client takes a factory so tests (and any page
// hosting a worker elsewhere) can supply their own.
export function moduleWorker(url) {
  return () => new Worker(url, { type: "module" });
}

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
  // Whether the worker stops a model download itself on cancel. Transformers.js
  // can't abort one, so for this worker a cancel during the download
  // terminates it: that is the only way to stop the transfer.
  cancelStopsDownload = false;

  constructor(workerFactory = moduleWorker(MODEL_WORKER_URL)) {
    this.worker = workerFactory();
    this.nextId = 1;
    this.pending = new Map();
    // A stopped client (worker crashed or disposed) never runs again; the
    // shared accessor below replaces it with a fresh worker.
    this.stopped = false;
    this.worker.addEventListener("message", (event) => this.onMessage(event.data));
    this.worker.addEventListener("error", (event) => {
      this.stop(new Error(event && event.message ? event.message : "AI worker failed."));
    });
  }

  // Terminate the worker and fail every pending call with `error`, detaching
  // their abort listeners so a later abort cannot post to the dead worker.
  stop(error) {
    if (this.stopped) return;
    this.stopped = true;
    this.worker.terminate();
    const pending = [...this.pending.values()];
    this.pending.clear();
    for (const { reject, cleanup } of pending) {
      if (cleanup) cleanup();
      reject(error);
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
    else {
      pending.reject(new Error(message.error || "AI inference failed."));
      // The worker's runtime trapped: retire it so the next call gets a fresh one.
      if (message.fatal) this.stop(new Error(message.error || "AI worker failed."));
    }
  }

  // Text-to-image: `prompt` → { blob, filename, backend }. Aborting `signal`
  // rejects with an AbortError: once the weights are loaded the worker stops
  // at the next image token; while they still download (stage "loading" or
  // "download") a worker that can't abort downloads is terminated, so no
  // further bytes arrive and the next call starts a fresh worker.
  generate(model, prompt, { onProgress = null, signal = null } = {}) {
    if (signal && signal.aborted) return Promise.reject(abortError());
    const id = this.nextId++;
    return new Promise((resolve, reject) => {
      let stage = null; // the worker's last reported stage
      const downloading = () => !this.cancelStopsDownload && (stage === "loading" || stage === "download");
      const onAbort = () => {
        if (downloading()) this.dispose();
        else this.worker.postMessage({ type: "cancel", id });
      };
      // The worker may have started loading before it read the cancel.
      const progress = (p) => {
        stage = p && p.stage;
        if (signal && signal.aborted && downloading()) this.dispose();
        else if (onProgress) onProgress(p);
      };
      const cleanup = signal ? () => signal.removeEventListener("abort", onAbort) : null;
      // A result the worker posted before it saw the cancel still counts as
      // cancelled: the caller has already moved on.
      const settle = (message) => (signal && signal.aborted ? reject(abortError()) : resolve(message));
      this.pending.set(id, { resolve: settle, reject, onProgress: progress, cleanup });
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

  // Terminates the worker (releasing everything it holds); pending calls
  // reject with an AbortError, as their caller has moved on.
  dispose() {
    this.stop(abortError());
  }
}

// SD-Turbo's worker speaks the same generate/cancel protocol, and aborts its
// own downloads on cancel.
export class SdWorkerClient extends ModelWorkerClient {
  cancelStopsDownload = true;

  constructor(workerFactory = moduleWorker(SD_WORKER_URL)) {
    super(workerFactory);
  }
}

function abortError() {
  return new DOMException("Image generation was cancelled.", "AbortError");
}

const ENGINE_CLIENTS = Object.freeze({
  janus: () => new ModelWorkerClient(),
  "sd-turbo": () => new SdWorkerClient(),
});

// One model resident at a time: each holds gigabytes of GPU and wasm memory,
// and terminating a worker is the only guaranteed release. Switching to
// another model — another engine, or another repo, revision or variant on
// the same engine — disposes the resident worker before the new one starts.
let resident = null; // { key, client }

function residentKey(engine, model) {
  return `${engine}|${model.id}@${model.revision}|${model.variant || ""}`;
}

function engineClient(engine, model) {
  const key = residentKey(engine, model);
  if (resident && resident.key === key && !resident.client.stopped) return resident.client;
  if (resident) resident.client.dispose();
  resident = { key, client: ENGINE_CLIENTS[engine]() };
  return resident.client;
}

// Generate an image locally from `prompt` on `engine` with a pinned
// text-to-image model (`model.task === "text-to-image"`). WebGPU only.
export function generateOn(engine, model, prompt, { onProgress = null, signal = null, nav = globalThis.navigator || {} } = {}) {
  if (!Object.hasOwn(ENGINE_CLIENTS, engine)) {
    return Promise.reject(new Error(`Unknown image engine "${engine}".`));
  }
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
  return engineClient(engine, model).generate(model, String(prompt).trim(), { onProgress, signal });
}

// The imagine page's pinned Janus model.
export function generateImage(model, prompt, options) {
  return generateOn("janus", model, prompt, options);
}

// Image-input models run on the Transformers.js worker, resident one model at
// a time like the image generators.
export function runModel(model, file, fields, onProgress) {
  if (!model || !model.task || !model.id || !model.revision) {
    return Promise.reject(new Error("This AI tool is missing its pinned model configuration."));
  }
  if (!compatibleDevices(model).length) {
    return Promise.reject(new Error("This browser supports neither WebGPU nor the WebAssembly fallback."));
  }
  return engineClient("janus", model).run(model, file, fields, onProgress);
}
