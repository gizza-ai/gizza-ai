// SD-Turbo worker logic, driven through createSdEngine with a fake ONNX
// Runtime, tokenizer, fetch and Cache Storage (real WebGPU can't run in Node).
import { test } from "node:test";
import assert from "node:assert/strict";

import { createSdEngine, inflateGzip, SD_CACHE } from "../tools/generator/assets/runtime/sd-worker.js";
import { SEQ_LEN, TIMESTEP } from "../tools/generator/assets/runtime/sd-core.js";

const REPO = "gizza-ai/sd-turbo-web";
const REV = "d2df2e40f2251622ec60053b0e9bfa8c64b7f14d";
const MODEL = {
  task: "text-to-image",
  engine: "sd-turbo",
  id: REPO,
  revision: REV,
  variant: "nof16q8",
  download_mb: 1735,
  output: { w: 16, h: 16 },
};
const WASM_URL = "https://gizza.test/tools/_model-runtime/ort-sd/ort-wasm-simd-threaded.asyncify.wasm.gz";
const WASM = new Uint8Array([0, 97, 115, 109, 1, 0, 0, 0, 42, 42, 42]);
const gzip = (bytes) => new Response(new Blob([bytes]).stream().pipeThrough(new CompressionStream("gzip")));
const url = (path) => `https://huggingface.co/${REPO}/resolve/${REV}/${path}`;

const FILES = {
  "tokenizer/tokenizer.json": 40,
  "nof16q8/text_encoder/model.onnx": 10,
  "nof16q8/text_encoder/model.onnx.data": 20,
  "nof16q8/unet/model.onnx": 10,
  "nof16q8/unet/model.onnx.data": 60,
  "nof16q8/vae_decoder/model.onnx": 10,
  "nof16q8/vae_decoder/model.onnx.data": 50,
};
const MANIFEST = {
  schema: 1,
  model: "stabilityai/sd-turbo",
  shared: [
    { path: "tokenizer/tokenizer.json", bytes: FILES["tokenizer/tokenizer.json"] },
    { path: "LICENSE.md", bytes: 999 },
    { path: "NOTICE", bytes: 99 },
  ],
  variants: {
    nof16q8: {
      files: Object.entries(FILES)
        .filter(([p]) => p.startsWith("nof16q8/"))
        .map(([path, bytes]) => ({ path, bytes })),
    },
    fp16: { files: [{ path: "fp16/unet/model.onnx", bytes: 5 }] },
  },
};
const TOTAL = Object.values(FILES).reduce((a, b) => a + b, 0);

function bodyFor(path) {
  if (path === "manifest.json") return new TextEncoder().encode(JSON.stringify(MANIFEST));
  if (path === "tokenizer/tokenizer.json") {
    const json = new TextEncoder().encode(JSON.stringify({ fake: "tokenizer" }));
    const out = new Uint8Array(FILES[path]).fill(32); // pad with spaces to its manifest size
    out.set(json);
    return out;
  }
  return new Uint8Array(FILES[path]).fill(7);
}

// Streams `bytes` in small chunks; `hold(path)` leaves that body open until aborted.
function fakeFetch({ hold = null, status = {} } = {}) {
  const calls = [];
  const fetch = async (u, { signal } = {}) => {
    if (u === WASM_URL) {
      calls.push({ path: "<wasm>", signal });
      return gzip(WASM);
    }
    const path = u.slice(url("").length);
    calls.push({ path, signal });
    if (status[path]) return new Response("nope", { status: status[path] });
    const bytes = bodyFor(path);
    let offset = 0;
    const body = new ReadableStream({
      pull(controller) {
        if (hold === path && offset >= 4) {
          return new Promise((_, reject) => {
            signal.addEventListener("abort", () => {
              const err = new DOMException("aborted", "AbortError");
              controller.error(err);
              reject(err);
            });
          });
        }
        if (offset >= bytes.length) return controller.close();
        controller.enqueue(bytes.slice(offset, offset + 4));
        offset += 4;
      },
    });
    return new Response(body, { status: 200 });
  };
  return { fetch, calls };
}

// Cache Storage stand-in: entries are buffered bytes, put() consumes the body.
function fakeCaches({ putError = null } = {}) {
  const store = new Map();
  const opened = [];
  const cache = {
    async match(key) {
      const e = store.get(key);
      return e ? new Response(e.bytes, { headers: e.headers }) : undefined;
    },
    async put(key, response) {
      if (putError && putError(key)) {
        await response.body.cancel();
        throw putError(key);
      }
      const bytes = new Uint8Array(await response.arrayBuffer());
      store.set(key, { bytes, headers: response.headers });
    },
    async delete(key) {
      return store.delete(key);
    },
  };
  return {
    store,
    opened,
    caches: {
      async open(name) {
        opened.push(name);
        return cache;
      },
    },
  };
}

class FakeTensor {
  constructor(type, data, dims) {
    Object.assign(this, { type, data, dims });
  }
}

const OUTPUTS = {
  text_encoder: { name: "last_hidden_state", dims: [1, SEQ_LEN, 4] },
  unet: { name: "out_sample", dims: null },
  vae_decoder: { name: "sample", dims: null },
};

function fakeOrt({ runError = null, onRun = null } = {}) {
  const log = { created: [], runs: [], live: 0, maxLive: 0 };
  const env = { webgpu: {}, wasm: {} };
  const ort = {
    env,
    Tensor: FakeTensor,
    InferenceSession: {
      async create(graph, options) {
        // The stage is identified by its weights' byte count (unique per file above).
        const stage = Object.keys(OUTPUTS)
          .find((s) => options.externalData[0].data.length === FILES[`nof16q8/${s}/model.onnx.data`]);
        log.created.push({ stage, options, graph, wasmBinary: env.wasm.wasmBinary });
        log.live += 1;
        log.maxLive = Math.max(log.maxLive, log.live);
        return {
          async run(feeds) {
            log.runs.push({ stage, feeds });
            if (onRun) await onRun(stage, feeds);
            if (runError && runError(stage)) throw runError(stage);
            const out = OUTPUTS[stage];
            if (stage === "text_encoder") {
              return { [out.name]: new FakeTensor("float32", new Float32Array(SEQ_LEN * 4), out.dims) };
            }
            if (stage === "unet") {
              return { [out.name]: new FakeTensor("float32", new Float32Array(feeds.sample.data.length), feeds.sample.dims) };
            }
            const { w, h } = MODEL.output;
            return { [out.name]: new FakeTensor("float32", new Float32Array(3 * w * h), [1, 3, h, w]) };
          },
          async release() {
            log.live -= 1;
          },
        };
      },
    },
  };
  return { ort, log };
}

class FakeTokenizer {
  static calls = [];
  constructor(json, config) {
    this.json = json;
    this.config = config;
  }
  _call(text, options) {
    FakeTokenizer.calls.push({ text, options, json: this.json });
    return { input_ids: [49406, 320, 49407] };
  }
}
// Transformers.js tokenizers are callable objects; mimic that.
function Tokenizer(json, config) {
  const t = new FakeTokenizer(json, config);
  return (text, options) => t._call(text, options);
}

function harness(overrides = {}) {
  const posted = [];
  const waiters = [];
  const post = (message) => {
    posted.push(message);
    for (const w of [...waiters]) if (w.pred(message)) { waiters.splice(waiters.indexOf(w), 1); w.resolve(message); }
  };
  const f = overrides.fetch || fakeFetch();
  const c = overrides.caches || fakeCaches();
  const o = overrides.ort || fakeOrt();
  const engine = createSdEngine({
    ort: o.ort,
    Tokenizer,
    fetch: f.fetch,
    caches: c.caches,
    gpu: overrides.gpu === undefined ? { requestAdapter: async () => ({}) } : overrides.gpu,
    encodePng: async (rgba, w, h) => new Blob([rgba.subarray(0, 4)], { type: "image/png" }),
    post,
    wasmUrl: WASM_URL,
  });
  const next = (pred) => {
    const hit = posted.find(pred);
    if (hit) return Promise.resolve(hit);
    return new Promise((resolve) => waiters.push({ pred, resolve }));
  };
  const final = (id) => next((m) => m.id === id && m.type !== "progress");
  return { engine, posted, next, final, fetch: f, caches: c, ort: o };
}

const generate = (h, id, prompt = "a red fox") => h.engine.handle({ type: "generate", id, model: MODEL, prompt });

test("a generation downloads every file once, runs the three stages one session at a time and posts a PNG", async () => {
  FakeTokenizer.calls = [];
  const h = harness();
  generate(h, 1);
  const result = await h.final(1);
  assert.equal(result.type, "result", JSON.stringify(result));
  assert.equal(result.filename, "generated.png");
  assert.equal(result.backend, "webgpu");
  assert.ok(result.blob instanceof Blob);

  // Weights live in their own Cache Storage, keyed by repo/revision/path.
  assert.deepEqual(new Set(h.caches.opened), new Set([SD_CACHE]));
  assert.deepEqual(
    [...h.caches.store.keys()].sort(),
    ["manifest.json", ...Object.keys(FILES)].map(url).sort(),
  );
  assert.ok(!h.fetch.calls.some((c) => c.path === "LICENSE.md"), "only files the run needs are fetched");

  // ORT's wasm binary arrives gzipped and is handed over inflated before the first session.
  assert.deepEqual(h.fetch.calls.filter((c) => c.path === "<wasm>").length, 1);
  assert.deepEqual(Array.from(h.ort.log.created[0].wasmBinary), Array.from(WASM));

  // Download progress: overall fraction from manifest sizes, never backwards, ends at 1.
  const downloads = h.posted.filter((m) => m.type === "progress" && m.progress.stage === "download");
  assert.ok(downloads.length > 1);
  for (let i = 1; i < downloads.length; i++) {
    assert.ok(downloads[i].progress.progress >= downloads[i - 1].progress.progress);
  }
  assert.equal(downloads.at(-1).progress.progress, 1);
  assert.equal(downloads.at(-1).progress.total, TOTAL);

  const counts = h.posted
    .filter((m) => m.type === "progress" && m.progress.stage === "generate")
    .map((m) => [m.progress.count, m.progress.total]);
  assert.deepEqual(counts, [[0, 3], [1, 3], [2, 3], [3, 3]]);

  // Sessions: one at a time, WebGPU, the spike's options, released after their run.
  const { log } = h.ort;
  assert.deepEqual(log.created.map((c) => c.stage), ["text_encoder", "unet", "vae_decoder"]);
  assert.equal(log.maxLive, 1);
  assert.equal(log.live, 0);
  for (const { options } of log.created) {
    assert.deepEqual(options.executionProviders, ["webgpu"]);
    assert.equal(options.graphOptimizationLevel, "all");
    assert.equal(options.externalData[0].path, "model.onnx.data");
  }

  // Feeds match the model IO.
  const [te, unet, vae] = log.runs;
  assert.equal(te.feeds.input_ids.type, "int32");
  assert.deepEqual(te.feeds.input_ids.dims, [1, SEQ_LEN]);
  assert.deepEqual(Array.from(te.feeds.input_ids.data.slice(0, 4)), [49406, 320, 49407, 0]);
  assert.deepEqual(unet.feeds.sample.dims, [1, 4, 2, 2]);
  assert.equal(unet.feeds.timestep.type, "int64");
  assert.deepEqual(Array.from(unet.feeds.timestep.data), [BigInt(TIMESTEP)]);
  assert.deepEqual(unet.feeds.encoder_hidden_states.dims, [1, SEQ_LEN, 4]);
  assert.deepEqual(vae.feeds.latent_sample.dims, [1, 4, 2, 2]);

  // The tokenizer is built from the repo's tokenizer.json and truncates like CLIP.
  assert.deepEqual(FakeTokenizer.calls[0].json, { fake: "tokenizer" });
  assert.equal(FakeTokenizer.calls[0].text, "a red fox");
  assert.equal(FakeTokenizer.calls[0].options.truncation, true);
  assert.equal(FakeTokenizer.calls[0].options.max_length, SEQ_LEN);
});

test("a second generation is served from Cache Storage without fetching", async () => {
  const h = harness();
  generate(h, 1);
  assert.equal((await h.final(1)).type, "result");
  const fetched = h.fetch.calls.length;
  generate(h, 2);
  assert.equal((await h.final(2)).type, "result");
  assert.equal(h.fetch.calls.length, fetched);
});

test("cancel during download aborts the in-flight fetch and replies cancelled at once", async () => {
  const hold = "nof16q8/unet/model.onnx.data";
  const h = harness({ fetch: fakeFetch({ hold }) });
  generate(h, 1);
  await h.next((m) => m.type === "progress" && m.progress.stage === "download" && m.progress.progress > 0.4);
  const call = h.fetch.calls.find((c) => c.path === hold);
  assert.ok(call, "the unet weights are downloading");
  h.engine.handle({ type: "cancel", id: 1 });
  // Replied synchronously, before the aborted fetch settles.
  assert.deepEqual(h.posted.at(-1), { type: "cancelled", id: 1 });
  assert.equal(call.signal.aborted, true);
  await new Promise((r) => setTimeout(r, 20));
  assert.deepEqual(h.posted.filter((m) => m.id === 1 && m.type !== "progress"), [{ type: "cancelled", id: 1 }]);
  assert.ok(!h.caches.store.has(url(hold)), "an aborted download is not cached");
  assert.equal(h.ort.log.created.length, 0);
});

test("a generation cancelled mid-pipeline never posts its result, and the next one waits for it", async () => {
  let release;
  let inUnet;
  const gate = new Promise((r) => { release = r; });
  const unetStarted = new Promise((r) => { inUnet = r; });
  const o = fakeOrt({ onRun: (stage) => (stage === "unet" ? (inUnet(), gate) : null) });
  const h = harness({ ort: o });
  generate(h, 1);
  await unetStarted; // the GPU is busy with job 1's unet and can't be interrupted
  h.engine.handle({ type: "cancel", id: 1 });
  assert.deepEqual(h.posted.at(-1), { type: "cancelled", id: 1 });
  generate(h, 2);
  await new Promise((r) => setTimeout(r, 10));
  assert.equal(o.log.maxLive, 1, "the next generation does not load while the cancelled one still runs");
  release();
  assert.equal((await h.final(2)).type, "result");
  assert.deepEqual(h.posted.filter((m) => m.id === 1 && m.type !== "progress"), [{ type: "cancelled", id: 1 }]);
  assert.equal(o.log.maxLive, 1);
});

test("a quota error from cache.put is a readable, fatal error", async () => {
  const quota = new DOMException("Quota exceeded.", "QuotaExceededError");
  const c = fakeCaches({ putError: (key) => (key.endsWith("unet/model.onnx.data") ? quota : null) });
  const h = harness({ caches: c });
  generate(h, 1);
  const reply = await h.final(1);
  assert.equal(reply.type, "error");
  assert.equal(reply.fatal, true);
  assert.match(reply.error, /storage/i);
  assert.match(reply.error, /1\.7 GB/);
});

test("a WebAssembly.RuntimeError is fatal and every session is released", async () => {
  const o = fakeOrt({ runError: (s) => (s === "unet" ? new WebAssembly.RuntimeError("memory access out of bounds") : null) });
  const h = harness({ ort: o });
  generate(h, 1);
  const reply = await h.final(1);
  assert.equal(reply.type, "error");
  assert.equal(reply.fatal, true);
  assert.match(reply.error, /memory/i);
  assert.equal(o.log.live, 0);
});

test("GPU device loss makes the failure fatal", async () => {
  let lose;
  const o = fakeOrt({
    runError: (s) => (s === "unet" ? new Error("Failed to run the unet") : null),
    onRun: async (stage) => {
      if (stage === "unet") {
        lose({ reason: "unknown", message: "GPU process crashed" });
        await new Promise((r) => setTimeout(r, 0));
      }
    },
  });
  o.ort.env.webgpu.device = { lost: new Promise((r) => { lose = r; }) };
  const h = harness({ ort: o });
  generate(h, 1);
  const reply = await h.final(1);
  assert.equal(reply.type, "error");
  assert.equal(reply.fatal, true);
  assert.match(reply.error, /GPU/);
});

test("ordinary failures are not fatal", async () => {
  const h = harness({ fetch: fakeFetch({ status: { "nof16q8/unet/model.onnx": 404 } }) });
  generate(h, 1);
  const reply = await h.final(1);
  assert.equal(reply.type, "error");
  assert.equal(reply.fatal, false);
  assert.match(reply.error, /404/);

  const h2 = harness();
  generate(h2, 2, "   ");
  const empty = await h2.final(2);
  assert.deepEqual([empty.type, empty.fatal], ["error", false]);
  assert.match(empty.error, /Describe the image/);

  const h3 = harness({ gpu: null });
  generate(h3, 3);
  const noGpu = await h3.final(3);
  assert.equal(noGpu.type, "error");
  assert.match(noGpu.error, /WebGPU/);
  assert.equal(h3.fetch.calls.length, 0, "nothing downloads without WebGPU");
});

test("an unknown variant is reported, not guessed", async () => {
  const h = harness();
  h.engine.handle({ type: "generate", id: 1, model: { ...MODEL, variant: "int4" }, prompt: "a fox" });
  const reply = await h.final(1);
  assert.equal(reply.type, "error");
  assert.match(reply.error, /int4/);
});

test("inflateGzip inflates a gzip body", async () => {
  const big = new Uint8Array(300_000).map((_, i) => i % 251);
  assert.deepEqual(await inflateGzip(gzip(big), new AbortController().signal), big);
  await assert.rejects(inflateGzip(new Response("x", { status: 404 }), new AbortController().signal), /404/);
});

test("inflateGzip stops when its signal aborts", async () => {
  const controller = new AbortController();
  const body = new ReadableStream({
    start(c) {
      c.enqueue(new Uint8Array([0x1f, 0x8b, 8, 0, 0, 0, 0, 0, 0, 3]));
    },
    pull() {
      return new Promise(() => {}); // the network stalls
    },
  });
  const pending = inflateGzip(new Response(body), controller.signal);
  setTimeout(() => controller.abort(), 5);
  await assert.rejects(pending, (e) => e.name === "AbortError");
});

test("the runtime binary loads once per worker", async () => {
  const h = harness();
  generate(h, 1);
  assert.equal((await h.final(1)).type, "result");
  const wasmFetches = h.fetch.calls.filter((c) => c.path === "<wasm>").length;
  generate(h, 2);
  assert.equal((await h.final(2)).type, "result");
  assert.equal(h.fetch.calls.filter((c) => c.path === "<wasm>").length, wasmFetches, "the runtime loads once per worker");
});
