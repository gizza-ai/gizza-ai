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
  const value = Number(progress.progress);
  if (Number.isFinite(value)) {
    const percent = value <= 1 ? value * 100 : value;
    return `Downloading AI model… ${Math.max(0, Math.min(100, Math.round(percent)))}%`;
  }
  return "Loading AI model…";
}

export async function validateModelFile(file, options = {}) {
  const maxBytes = options.maxBytes || 20 * 1024 * 1024;
  const maxPixels = options.maxPixels || 40_000_000;
  const supportedTypes = new Set(["image/png", "image/jpeg", "image/webp"]);
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
    this.worker.addEventListener("message", (event) => this.onMessage(event.data));
    this.worker.addEventListener("error", (event) => {
      const message = event && event.message ? event.message : "AI worker failed.";
      for (const { reject } of this.pending.values()) reject(new Error(message));
      this.pending.clear();
    });
  }

  onMessage(message) {
    const pending = this.pending.get(message && message.id);
    if (!pending) return;
    if (message.type === "progress") {
      if (pending.onProgress) pending.onProgress(message.progress);
      return;
    }
    this.pending.delete(message.id);
    if (message.type === "result") pending.resolve(message);
    else pending.reject(new Error(message.error || "AI inference failed."));
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
    this.worker.terminate();
    for (const { reject } of this.pending.values()) reject(new Error("AI worker stopped."));
    this.pending.clear();
  }
}

let sharedClient = null;

export function runModel(model, file, fields, onProgress) {
  if (!model || !model.task || !model.id || !model.revision) {
    return Promise.reject(new Error("This AI tool is missing its pinned model configuration."));
  }
  if (!compatibleDevices(model).length) {
    return Promise.reject(new Error("This browser supports neither WebGPU nor the WebAssembly fallback."));
  }
  if (!sharedClient) sharedClient = new ModelWorkerClient();
  return sharedClient.run(model, file, fields, onProgress);
}
