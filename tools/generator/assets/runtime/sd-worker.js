// SD-Turbo text-to-image in its own module worker, on onnxruntime-web's
// WebGPU build (ort-sd/, version-matched to the published ONNX export) rather
// than the ONNX Runtime bundled in Transformers.js, which Janus keeps using.
// Transformers.js is used here only for its CLIP tokenizer.
//
// Request:  { type: "generate", id, model, prompt }   (model: an image-models.js
//           descriptor with engine "sd-turbo"),  { type: "cancel", id }
// Replies:  progress { stage: "download", progress: 0..1, loaded, total } over
//           every file the run needs (sizes from the repo's manifest.json),
//           progress { stage: "generate", count, total: 3 } (text, unet, vae),
//           then { type: "result", id, blob, filename, backend } |
//           { type: "cancelled", id } | { type: "error", id, error, fatal }
//           (fatal: this worker can't run again; the client replaces it).
//           Cancel replies "cancelled" at once and aborts any download; no
//           other reply for that id follows.

import {
  SEQ_LEN,
  TIMESTEP,
  initLatents,
  latentsForVae,
  noise,
  padTokens,
  scaleModelInput,
  stepEpsilon,
  toImageData,
} from "./sd-core.js";

// Cache Storage bucket for SD weights, keyed by their pinned resolve URL
// (repo + revision + path), so a new revision never reads stale files.
export const SD_CACHE = "gizza-sd-models";

const STAGES = ["text_encoder", "unet", "vae_decoder"];
const TOKENIZER = "tokenizer/tokenizer.json";

// The spike's session options (browser-proven on a no-shader-f16 GPU).
// graphOptimizationLevel is ORT's default, set explicitly so it can't drift.
const SESSION_OPTIONS = Object.freeze({
  executionProviders: ["webgpu"],
  graphOptimizationLevel: "all",
  enableMemPattern: false,
  enableCpuMemArena: false,
  extra: {
    session: {
      disable_prepacking: "1",
      use_device_allocator_for_initializers: "1",
      use_ort_model_bytes_directly: "1",
      use_ort_model_bytes_for_initializers: "1",
    },
  },
});

const WEBGPU_REQUIRED = "Image generation needs WebGPU, which this browser or device doesn't provide.";

const fileUrl = (model, path) => `https://huggingface.co/${model.id}/resolve/${model.revision}/${path}`;

async function readAll(stream, signal) {
  const reader = (signal ? stream.pipeThrough(new TransformStream(), { signal }) : stream).getReader();
  const chunks = [];
  let size = 0;
  for (;;) {
    const { done, value } = await reader.read();
    if (done) break;
    chunks.push(value);
    size += value.byteLength;
  }
  const out = new Uint8Array(size);
  let offset = 0;
  for (const chunk of chunks) {
    out.set(chunk, offset);
    offset += chunk.byteLength;
  }
  return out;
}

const startsWith = (bytes, magic) => magic.every((b, i) => bytes[i] === b);
const GZIP_MAGIC = [0x1f, 0x8b];
const WASM_MAGIC = [0x00, 0x61, 0x73, 0x6d];

// ORT's wasm binary from its .gz. A server that labels the file
// `Content-Encoding: gzip` has the browser inflate it on the way in, so the
// bytes are sniffed: gzip is inflated here, wasm is used as-is. Aborting
// `signal` stops both the download and the inflate.
export async function loadWasmBinary(response, signal) {
  if (!response.ok) throw new Error(`HTTP ${response.status}`);
  const bytes = await readAll(response.body, signal);
  if (startsWith(bytes, GZIP_MAGIC)) {
    return readAll(new Blob([bytes]).stream().pipeThrough(new DecompressionStream("gzip")), signal);
  }
  if (startsWith(bytes, WASM_MAGIC)) return bytes;
  throw new Error("the runtime binary is neither gzip nor WebAssembly");
}

class Cancelled extends Error {}

function isQuotaError(error) {
  return error && error.name === "QuotaExceededError";
}

// deps: { ort, Tokenizer, fetch, caches, gpu, encodePng, post, wasmUrl }
//   Tokenizer(tokenizerJSON, config) → callable Transformers.js tokenizer
//   encodePng(rgba, w, h) → Promise<Blob>
//   wasmUrl: ORT's gzipped wasm binary, inflated into ort.env.wasm.wasmBinary
export function createSdEngine(deps) {
  const { ort, Tokenizer, fetch, caches, gpu, encodePng, post, wasmUrl } = deps;
  const jobs = new Map(); // id → { controller, settled }
  // Generations run one after another: a cancelled run can't stop its
  // in-flight session.run, so the next one waits rather than doubling the
  // GPU and wasm-heap footprint.
  let queue = Promise.resolve();
  let deviceLost = null;
  let watchingDevice = false;

  function handle(request) {
    if (request.type === "generate") {
      const job = { controller: new AbortController(), settled: false };
      jobs.set(request.id, job);
      queue = queue.then(() => run(request, job));
    } else if (request.type === "cancel") {
      const job = jobs.get(request.id);
      if (job && !job.settled) {
        job.settled = true;
        job.controller.abort();
        post({ type: "cancelled", id: request.id });
      }
    }
  }

  async function run({ id, model, prompt }, job) {
    const signal = job.controller.signal;
    const progress = (p) => {
      if (!job.settled) post({ type: "progress", id, progress: p });
    };
    const check = () => {
      if (signal.aborted) throw new Cancelled();
    };
    try {
      if (job.settled) return;
      if (!model || model.task !== "text-to-image" || model.engine !== "sd-turbo") {
        throw new Error(`Unsupported image model: ${model && model.engine}`);
      }
      const text = String(prompt || "").trim();
      if (!text) throw new Error("Describe the image you want first.");
      const adapter = gpu && typeof gpu.requestAdapter === "function" ? await gpu.requestAdapter() : null;
      if (!adapter) throw new Error(WEBGPU_REQUIRED);
      progress({ stage: "loading", device: "webgpu" });

      const cache = await caches.open(SD_CACHE);
      const manifest = JSON.parse(new TextDecoder().decode(await cachedBytes(cache, model, "manifest.json", null, signal)));
      const files = runFiles(manifest, model);
      await download(cache, model, files, signal, progress);
      check();
      await loadRuntime(signal);
      check();

      progress({ stage: "generate", count: 0, total: STAGES.length });
      const tokenizer = Tokenizer(JSON.parse(new TextDecoder().decode(await readCached(cache, model, files, TOKENIZER))), {});
      // Encoded whole: Transformers.js truncation drops the end token, so
      // padTokens truncates (keeping it) and pads like CLIPTokenizer.
      const encoded = tokenizer(text, { return_tensor: false });
      const ids = padTokens(Array.from(encoded.input_ids, Number));

      const stage = async (name, feeds) => {
        const out = await runStage(cache, model, files, name, feeds, signal);
        progress({ stage: "generate", count: STAGES.indexOf(name) + 1, total: STAGES.length });
        return out;
      };
      const { last_hidden_state: hidden } = await stage("text_encoder", {
        input_ids: new ort.Tensor("int32", ids, [1, SEQ_LEN]),
      });

      const { w, h } = model.output;
      const latentDims = [1, 4, h / 8, w / 8];
      const raw = noise(crypto.getRandomValues(new Uint32Array(1))[0], 4 * (h / 8) * (w / 8));
      const { out_sample: eps } = await stage("unet", {
        sample: new ort.Tensor("float32", scaleModelInput(raw), latentDims),
        timestep: new ort.Tensor("int64", BigInt64Array.of(BigInt(TIMESTEP)), [1]),
        encoder_hidden_states: new ort.Tensor("float32", hidden.data, hidden.dims),
      });

      const x0 = stepEpsilon(initLatents(raw), eps.data);
      const { sample } = await stage("vae_decoder", {
        latent_sample: new ort.Tensor("float32", latentsForVae(x0), latentDims),
      });
      const blob = await encodePng(toImageData(sample.data, w, h), w, h);
      if (job.settled) return;
      job.settled = true;
      post({ type: "result", id, blob, filename: "generated.png", backend: "webgpu" });
    } catch (error) {
      if (job.settled) return; // cancelled: already answered
      job.settled = true;
      post({ type: "error", id, ...describe(error, model) });
    } finally {
      jobs.delete(id);
    }
  }

  function runFiles(manifest, model) {
    if (!manifest || manifest.schema !== 1 || !manifest.variants) {
      throw new Error("The image model's manifest is not one this page understands.");
    }
    const variant = manifest.variants[model.variant];
    if (!variant) throw new Error(`The image model has no "${model.variant}" build.`);
    const tokenizer = manifest.shared.find((f) => f.path === TOKENIZER);
    if (!tokenizer) throw new Error("The image model's manifest lists no tokenizer.");
    return [tokenizer, ...variant.files];
  }

  // Every file the run needs into Cache Storage, reporting one overall
  // fraction over their manifest sizes.
  async function download(cache, model, files, signal, progress) {
    const total = files.reduce((sum, f) => sum + f.bytes, 0);
    let done = 0;
    let last = -1;
    const report = (loaded) => {
      const fraction = Math.min(1, loaded / total);
      // A few hundred updates over gigabytes, not one per network chunk.
      if (fraction < 1 && fraction - last < 0.002) return;
      last = fraction;
      progress({ stage: "download", progress: fraction, loaded, total });
    };
    for (const file of files) {
      await cachedBytes(cache, model, file.path, file, signal, (got) => report(done + got), false);
      done += file.bytes;
      report(done);
    }
  }

  // Ensures `path` is cached (streaming it there on a miss) and, when `read`,
  // returns its bytes. `file` (manifest entry) pins the expected size.
  async function cachedBytes(cache, model, path, file, signal, onBytes = () => {}, read = true) {
    const key = fileUrl(model, path);
    const hit = await cache.match(key);
    if (hit && (!file || Number(hit.headers.get("content-length")) === file.bytes)) {
      return read ? new Uint8Array(await hit.arrayBuffer()) : null;
    }
    const response = await fetch(key, { signal });
    if (!response.ok) throw new Error(`Couldn't download the image model (${path}: HTTP ${response.status}).`);
    let got = 0;
    const counted = response.body.pipeThrough(
      new TransformStream({
        transform(chunk, controller) {
          got += chunk.byteLength;
          onBytes(got);
          controller.enqueue(chunk);
        },
      }),
      { signal },
    );
    const headers = { "content-type": "application/octet-stream" };
    if (file) headers["content-length"] = String(file.bytes);
    await cache.put(key, new Response(counted, { headers }));
    if (file && got !== file.bytes) {
      await cache.delete(key);
      throw new Error(`The image model download was incomplete (${path}).`);
    }
    if (!read) return null;
    const stored = await cache.match(key);
    return new Uint8Array(await stored.arrayBuffer());
  }

  async function readCached(cache, model, files, path) {
    const file = files.find((f) => f.path === path);
    const hit = await cache.match(fileUrl(model, path));
    if (!hit) throw new Error("The browser dropped the downloaded image model; try again.");
    const bytes = new Uint8Array(await hit.arrayBuffer());
    if (bytes.length !== file.bytes) throw new Error(`The cached image model is damaged (${path}); try again.`);
    return bytes;
  }

  // The wasm binary ships gzipped (see main.rs); ORT gets it inflated, so it
  // never fetches a .wasm itself. Loaded once per worker, before any session.
  async function loadRuntime(signal) {
    if (ort.env.wasm.wasmBinary) return;
    const response = await fetch(wasmUrl, { signal });
    let bytes;
    try {
      bytes = await loadWasmBinary(response, signal);
    } catch (error) {
      if (signal.aborted) throw error;
      throw new Error(`Couldn't load the image runtime (${error.message || error}).`);
    }
    ort.env.wasm.wasmBinary = bytes;
  }

  // One session at a time: created from cached bytes, released after its run.
  async function runStage(cache, model, files, name, feeds, signal) {
    const base = `${model.variant}/${name}`;
    const graph = await readCached(cache, model, files, `${base}/model.onnx`);
    const data = await readCached(cache, model, files, `${base}/model.onnx.data`);
    if (signal.aborted) throw new Cancelled();
    const session = await ort.InferenceSession.create(graph, {
      ...SESSION_OPTIONS,
      externalData: [{ path: "model.onnx.data", data }],
    });
    watchDevice();
    try {
      if (signal.aborted) throw new Cancelled();
      return await session.run(feeds);
    } finally {
      await session.release();
    }
  }

  // ORT creates its GPUDevice with the first WebGPU session; once that device
  // is lost, nothing in this worker can run on the GPU again.
  function watchDevice() {
    if (watchingDevice) return;
    const device = ort.env.webgpu && ort.env.webgpu.device;
    if (!device) return;
    watchingDevice = true;
    Promise.resolve(device).then((d) => {
      if (d && d.lost) d.lost.then((info) => { deviceLost = info || {}; });
    });
  }

  function describe(error, model) {
    if (isQuotaError(error)) {
      const gb = model && model.download_mb ? ` (about ${(model.download_mb / 1000).toFixed(1)} GB)` : "";
      return {
        error: `This browser doesn't have enough free storage to keep the image model${gb}. Free up disk space or clear site data, then try again.`,
        fatal: true,
      };
    }
    if (typeof WebAssembly !== "undefined" && error instanceof WebAssembly.RuntimeError) {
      return {
        error: "The image model ran out of memory on this device. Try again; if it keeps failing, this device can't run it.",
        fatal: true,
      };
    }
    if (deviceLost) {
      return { error: "The GPU stopped responding while generating. Try again.", fatal: true };
    }
    const message = typeof error === "string" ? error : (error && error.message) || "Image generation failed.";
    return { error: message, fatal: false };
  }

  return { handle };
}

// ---- Worker boot -------------------------------------------------------------
// Only inside a worker: Node tests import createSdEngine without the browser
// runtimes. The listener is registered synchronously so no message posted
// while the runtimes load is lost; each waits for the engine in order.
if (typeof WorkerGlobalScope !== "undefined" && self instanceof WorkerGlobalScope) {
  const runtime = new URL("./ort-sd/", import.meta.url);
  const engine = Promise.all([import("./ort-sd/ort.webgpu.min.mjs"), import("./transformers.min.js")]).then(
    ([ort, transformers]) => {
      // Only the JS factory loads from here; the binary is handed over as
      // wasmBinary (inflated from its .gz), so ORT never fetches a .wasm.
      ort.env.wasm.wasmPaths = { mjs: new URL("ort-wasm-simd-threaded.asyncify.mjs", runtime).href };
      ort.env.wasm.numThreads = 1; // no cross-origin isolation on these pages
      ort.env.wasm.proxy = false;
      ort.env.webgpu.powerPreference = "high-performance";
      const { CLIPTokenizer } = transformers;
      return createSdEngine({
        ort,
        Tokenizer: (json, config) => new CLIPTokenizer(json, config),
        fetch: (...args) => self.fetch(...args),
        caches: self.caches,
        gpu: self.navigator.gpu,
        encodePng: async (rgba, w, h) => {
          const canvas = new OffscreenCanvas(w, h);
          canvas.getContext("2d").putImageData(new ImageData(rgba, w, h), 0, 0);
          return canvas.convertToBlob({ type: "image/png" });
        },
        post: (message) => self.postMessage(message),
        wasmUrl: new URL("ort-wasm-simd-threaded.asyncify.wasm.gz", runtime).href,
      });
    },
  );
  self.addEventListener("message", (event) => {
    const request = event.data || {};
    engine.then(
      (e) => e.handle(request),
      (error) => {
        if (request.type !== "generate") return;
        self.postMessage({
          type: "error",
          id: request.id,
          error: `The image runtime failed to load (${(error && error.message) || error}).`,
          fatal: true,
        });
      },
    );
  });
}
