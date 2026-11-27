import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import {
  IMAGE_MODELS,
  adapterInfo,
  defaultModel,
  deviceFit,
  generateWith,
  resolveVariant,
} from "../tools/generator/assets/runtime/image-models.js";

const byId = (id) => IMAGE_MODELS.find((m) => m.id === id);

test("defaultModel is SD-Turbo, resolved to nof16q8, without shader-f16", () => {
  const info = { webgpu: true, shaderF16: false, maxBufferSize: 2 ** 31 };
  const m = defaultModel(info);
  assert.equal(m.engine, "sd-turbo");
  assert.equal(resolveVariant(m, info).name, "nof16q8");
});

test("shader-f16 does not select fp16 while it is unverified", () => {
  const info = { webgpu: true, shaderF16: true, maxBufferSize: 2 ** 31 };
  const m = defaultModel(info);
  assert.equal(m.engine, "sd-turbo");
  assert.equal(m.variants.fp16.verified, false);
  assert.equal(resolveVariant(m, info).name, "nof16q8");
});

test("defaultModel falls back to Janus when SD-Turbo is risky on a small-buffer adapter", () => {
  const info = { webgpu: true, shaderF16: false, maxBufferSize: 2 ** 29 };
  assert.equal(deviceFit(byId("sd-turbo"), info).level, "risky");
  assert.equal(defaultModel(info).id, "janus-pro-1b");
});

test("defaultModel still returns the recommended entry without webgpu", () => {
  assert.equal(defaultModel({ webgpu: false, shaderF16: false, maxBufferSize: 0 }).id, "sd-turbo");
});

test("deviceFit is unsupported without webgpu", () => {
  for (const m of IMAGE_MODELS) {
    const fit = deviceFit(m, { webgpu: false, shaderF16: false, maxBufferSize: 0 });
    assert.equal(fit.level, "unsupported");
    assert.ok(fit.note);
  }
});

test("deviceFit is ok on a capable adapter and risky on a small buffer limit", () => {
  const sd = byId("sd-turbo");
  assert.equal(deviceFit(sd, { webgpu: true, shaderF16: false, maxBufferSize: 2 ** 31 }).level, "ok");
  assert.equal(deviceFit(sd, { webgpu: true, shaderF16: false, maxBufferSize: 2 ** 20 }).level, "risky");
});

test("Janus descriptor matches the imagine tool's page model", () => {
  const meta = readFileSync(new URL("../blocks/imagine/page/meta.toml", import.meta.url), "utf8");
  const field = (k) => meta.match(new RegExp(`^${k}\\s*=\\s*"?([^"\\n]+)"?`, "m"))[1].trim();
  const janus = byId("janus-pro-1b");
  const info = { webgpu: true, shaderF16: true, maxBufferSize: 2 ** 31 };
  const d = resolveVariant(janus, info).descriptor;
  assert.equal(resolveVariant(janus, info).name, "f16");
  assert.deepEqual(
    { task: d.task, id: d.id, revision: d.revision, download_mb: d.download_mb },
    { task: field("task"), id: field("id"), revision: field("revision"), download_mb: Number(field("download_mb")) },
  );
  assert.equal(d.engine, "janus");
  assert.equal(d.variant, "f16");
  assert.deepEqual(d.output, { w: 384, h: 384 });
  assert.equal(resolveVariant(janus, { ...info, shaderF16: false }).name, "f32");
});

test("resolveVariant returns null when no variant fits, and for no webgpu", () => {
  assert.equal(resolveVariant(byId("sd-turbo"), { webgpu: false, shaderF16: false, maxBufferSize: 0 }), null);
});

test("every entry has license name and url, and shape matches custom entries", () => {
  for (const m of IMAGE_MODELS) {
    assert.ok(m.license.name && m.license.url, m.id);
    for (const k of ["id", "label", "tagline", "engine", "repo", "revision", "output", "variants"]) {
      assert.ok(m[k], `${m.id}.${k}`);
    }
    for (const v of Object.values(m.variants)) {
      assert.equal(typeof v.requires.shaderF16, "boolean");
      assert.equal(typeof v.download_mb, "number");
      assert.equal(typeof v.verified, "boolean");
    }
  }
  assert.equal(IMAGE_MODELS.filter((m) => m.recommended).length, 1);
});

test("adapterInfo reads webgpu, shader-f16 and maxBufferSize", async () => {
  const nav = {
    gpu: {
      requestAdapter: async () => ({
        features: new Set(["shader-f16"]),
        limits: { maxBufferSize: 123 },
      }),
    },
  };
  assert.deepEqual(await adapterInfo(nav), { webgpu: true, shaderF16: true, maxBufferSize: 123 });
  assert.deepEqual(await adapterInfo({}), { webgpu: false, shaderF16: false, maxBufferSize: 0 });
  assert.deepEqual(await adapterInfo({ gpu: { requestAdapter: async () => null } }), {
    webgpu: false,
    shaderF16: false,
    maxBufferSize: 0,
  });
});

// ---- generateWith: engine routing, one engine resident ----------------------

class RoutedWorker {
  static all = [];
  listeners = new Map();
  posted = [];
  terminated = false;
  constructor(url, options) {
    this.url = String(url);
    this.options = options;
    RoutedWorker.all.push(this);
  }
  addEventListener(type, fn) {
    this.listeners.set(type, fn);
  }
  postMessage(message) {
    this.posted.push(message);
  }
  emit(data) {
    this.listeners.get("message")?.({ data });
  }
  terminate() {
    this.terminated = true;
  }
}

const capable = { webgpu: true, shaderF16: false, maxBufferSize: 2 ** 31 };
const SD = resolveVariant(byId("sd-turbo"), capable).descriptor;
const JANUS = resolveVariant(byId("janus-pro-1b"), capable).descriptor;
const nav = { gpu: {} };

async function withWorkers(fn) {
  const NativeWorker = globalThis.Worker;
  RoutedWorker.all = [];
  globalThis.Worker = RoutedWorker;
  try {
    await fn(RoutedWorker.all);
  } finally {
    // Retire whatever is resident so the next test starts from no engine.
    for (const w of RoutedWorker.all) {
      if (!w.terminated) w.listeners.get("error")?.({ message: "test cleanup" });
    }
    globalThis.Worker = NativeWorker;
  }
}

const isEngineWorker = (w, file) => w.url.endsWith(`/_model-runtime/${file}`) && w.options.type === "module";

test("generateWith routes each engine to its own worker", async () => {
  await withWorkers(async (workers) => {
    const sd = generateWith(SD, " a red fox ", { nav });
    assert.equal(workers.length, 1);
    assert.ok(isEngineWorker(workers[0], "sd-worker.js"), workers[0].url);
    const msg = workers[0].posted[0];
    assert.deepEqual({ type: msg.type, model: msg.model, prompt: msg.prompt }, { type: "generate", model: SD, prompt: "a red fox" });
    workers[0].emit({ type: "result", id: msg.id, blob: new Blob(["png"]), filename: "generated.png", backend: "webgpu" });
    assert.equal((await sd).filename, "generated.png");

    const again = generateWith(SD, "a cat", { nav });
    assert.equal(workers.length, 1, "the resident engine is reused");
    workers[0].emit({ type: "result", id: workers[0].posted[1].id, blob: new Blob(["png"]), filename: "generated.png", backend: "webgpu" });
    await again;
  });
});

test("switching to SD-Turbo terminates the Janus worker first; its generation never resolves", async () => {
  await withWorkers(async (workers) => {
    const janus = generateWith(JANUS, "a red fox", { nav });
    assert.ok(isEngineWorker(workers[0], "model-worker.js"), workers[0].url);
    const janusId = workers[0].posted[0].id;
    const sd = generateWith(SD, "a red fox", { nav });
    assert.equal(workers[0].terminated, true, "Janus is gone before SD starts");
    assert.ok(isEngineWorker(workers[1], "sd-worker.js"));
    await assert.rejects(janus, (e) => e.name === "AbortError");
    // A result the dead worker had in flight resolves nothing.
    workers[0].emit({ type: "result", id: janusId, blob: new Blob(["png"]), filename: "generated.png", backend: "webgpu" });
    workers[1].emit({ type: "result", id: workers[1].posted[0].id, blob: new Blob(["png"]), filename: "sd.png", backend: "webgpu" });
    assert.equal((await sd).filename, "sd.png");
  });
});

test("switching to Janus terminates the SD-Turbo worker first", async () => {
  await withWorkers(async (workers) => {
    const sd = generateWith(SD, "a red fox", { nav });
    const janus = generateWith(JANUS, "a red fox", { nav });
    assert.equal(workers[0].terminated, true);
    assert.ok(isEngineWorker(workers[1], "model-worker.js"));
    await assert.rejects(sd, (e) => e.name === "AbortError");
    workers[1].emit({ type: "result", id: workers[1].posted[0].id, blob: new Blob(["png"]), filename: "generated.png", backend: "webgpu" });
    await janus;
  });
});

test("a fatal SD error retires its worker and the next generation gets a fresh one", async () => {
  await withWorkers(async (workers) => {
    const first = generateWith(SD, "a red fox", { nav });
    workers[0].emit({
      type: "error",
      id: workers[0].posted[0].id,
      error: "This browser doesn't have enough free storage to keep the image model (about 1.7 GB).",
      fatal: true,
    });
    await assert.rejects(first, /enough free storage/);
    assert.equal(workers[0].terminated, true);
    const second = generateWith(SD, "a red fox", { nav });
    assert.equal(workers.length, 2);
    assert.ok(isEngineWorker(workers[1], "sd-worker.js"));
    workers[1].emit({ type: "result", id: workers[1].posted[0].id, blob: new Blob(["png"]), filename: "generated.png", backend: "webgpu" });
    await second;
  });
});

test("generateWith forwards progress and cancels through the signal", async () => {
  await withWorkers(async (workers) => {
    const seen = [];
    const controller = new AbortController();
    const p = generateWith(SD, "a red fox", { nav, signal: controller.signal, onProgress: (x) => seen.push(x) });
    const { id } = workers[0].posted[0];
    workers[0].emit({ type: "progress", id, progress: { stage: "download", progress: 0.5 } });
    controller.abort();
    assert.deepEqual(workers[0].posted[1], { type: "cancel", id });
    workers[0].emit({ type: "cancelled", id });
    await assert.rejects(p, (e) => e.name === "AbortError");
    assert.deepEqual(seen, [{ stage: "download", progress: 0.5 }]);
  });
});

test("generateWith rejects without WebGPU, a pinned descriptor, a known engine or a prompt", async () => {
  await withWorkers(async (workers) => {
    await assert.rejects(generateWith(SD, "fox", { nav: {} }), /WebGPU/);
    await assert.rejects(generateWith({ ...SD, revision: "" }, "fox", { nav }), /pinned model/);
    await assert.rejects(generateWith({ ...SD, engine: "dall-e" }, "fox", { nav }), /dall-e/);
    await assert.rejects(generateWith({ ...SD, engine: "toString" }, "fox", { nav }), /toString/);
    await assert.rejects(generateWith({ ...SD, task: "background-removal" }, "fox", { nav }), /text-to-image/);
    await assert.rejects(generateWith(SD, "   ", { nav }), /Describe the image/);
    assert.equal(workers.length, 0, "no worker starts for a rejected request");
  });
});
