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

test("resolveVariant prefers verified, falls back to unverified, else null", () => {
  const f16 = { webgpu: true, shaderF16: true, maxBufferSize: 2 ** 31 };
  assert.equal(resolveVariant(byId("sd-turbo"), f16).name, "nof16q8");
  const custom = {
    ...byId("sd-turbo"),
    id: "custom:x/y@abc",
    variants: { fp16: { requires: { shaderF16: true }, download_mb: 1, verified: false } },
  };
  assert.equal(resolveVariant(custom, f16).name, "fp16");
  assert.equal(resolveVariant(custom, { ...f16, shaderF16: false }), null);
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

test("a license summary that cites the Acceptable Use Policy links it as an https aup url", () => {
  assert.equal(byId("sd-turbo").license.aup, "https://stability.ai/use-policy");
  for (const m of IMAGE_MODELS) {
    if (/Acceptable Use Policy/.test(m.license.summary)) assert.ok(m.license.aup, `${m.id} cites the AUP without linking it`);
    if (m.license.aup !== undefined) assert.equal(new URL(m.license.aup).protocol, "https:", m.id);
  }
  assert.equal(byId("janus-pro-1b").license.aup, undefined, "aup is optional");
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

// The resident slot admits calls asynchronously (it may wait for another
// model, and reads the adapter for Janus): let it settle before inspecting.
const settle = () => new Promise((r) => setImmediate(r));
const result = (w, i = 0, filename = "generated.png") =>
  w.emit({ type: "result", id: w.posted[i].id, blob: new Blob(["png"]), filename, backend: "webgpu" });

test("generateWith routes each engine to its own worker", async () => {
  await withWorkers(async (workers) => {
    const sd = generateWith(SD, " a red fox ", { nav });
    await settle();
    assert.equal(workers.length, 1);
    assert.ok(isEngineWorker(workers[0], "sd-worker.js"), workers[0].url);
    const msg = workers[0].posted[0];
    assert.deepEqual({ type: msg.type, model: msg.model, prompt: msg.prompt }, { type: "generate", model: SD, prompt: "a red fox" });
    result(workers[0]);
    assert.equal((await sd).filename, "generated.png");

    const again = generateWith(SD, "a cat", { nav });
    await settle();
    assert.equal(workers.length, 1, "the resident engine is reused");
    result(workers[0], 1);
    await again;
  });
});

for (const [from, to, fromFile, toFile] of [
  ["Janus", "SD-Turbo", "model-worker.js", "sd-worker.js"],
  ["SD-Turbo", "Janus", "sd-worker.js", "model-worker.js"],
]) {
  test(`starting ${to} while ${from} generates lets ${from} finish, then replaces its worker`, async () => {
    const pick = (name) => (name === "Janus" ? JANUS : SD);
    await withWorkers(async (workers) => {
      const running = generateWith(pick(from), "a red fox", { nav });
      await settle();
      assert.ok(isEngineWorker(workers[0], fromFile), workers[0].url);
      const seen = [];
      const next = generateWith(pick(to), "a blue bird", { nav, onProgress: (p) => seen.push(p) });
      await settle();
      assert.equal(workers.length, 1, `${to} waits for ${from}'s generation`);
      assert.equal(workers[0].terminated, false, `${from}'s generation is not cancelled`);
      assert.deepEqual(seen, [{ stage: "queued" }]);
      result(workers[0], 0, "first.png");
      assert.equal((await running).filename, "first.png", `${from}'s generation completes`);
      await settle();
      assert.equal(workers[0].terminated, true, `${from} is gone before ${to} starts`);
      assert.ok(isEngineWorker(workers[1], toFile));
      result(workers[1], 0, "second.png");
      assert.equal((await next).filename, "second.png");
    });
  });
}

test("a queued call cancelled while it waits leaves the running model alone and never starts", async () => {
  await withWorkers(async (workers) => {
    const running = generateWith(JANUS, "a red fox", { nav });
    await settle();
    const controller = new AbortController();
    const queued = generateWith(SD, "a blue bird", { nav, signal: controller.signal });
    await settle();
    controller.abort();
    await assert.rejects(queued, (e) => e.name === "AbortError");
    await settle();
    assert.equal(workers.length, 1, "SD never starts");
    assert.equal(workers[0].terminated, false);
    result(workers[0]);
    await running;
  });
});

test("a call that arrives behind a queued switch waits its turn instead of joining the outgoing model", async () => {
  await withWorkers(async (workers) => {
    const first = generateWith(JANUS, "a red fox", { nav });
    await settle();
    const sd = generateWith(SD, "a blue bird", { nav });
    const janusAgain = generateWith(JANUS, "a cat", { nav });
    await settle();
    assert.equal(workers[0].posted.length, 1, "nothing new reaches Janus while SD waits");
    result(workers[0]);
    await first;
    await settle();
    assert.ok(isEngineWorker(workers[1], "sd-worker.js"));
    result(workers[1]);
    await sd;
    await settle();
    assert.ok(isEngineWorker(workers[2], "model-worker.js"), "Janus comes back after SD");
    result(workers[2]);
    await janusAgain;
  });
});

test("switching models on the same engine replaces its worker; the same model reuses it", async () => {
  await withWorkers(async (workers) => {
    const other = { ...JANUS, id: "acme/janus-web", revision: "0123456789abcdef0123456789abcdef01234567" };
    const steps = [
      [JANUS, 1, "first model"],
      [JANUS, 1, "same model: reused"],
      [other, 2, "another repo on the same engine: replaced"],
      [{ ...other, variant: other.variant === "f16" ? "f32" : "f16" }, 3, "another variant: replaced"],
      [{ ...JANUS, revision: "fedcba9876543210fedcba9876543210fedcba98" }, 4, "another revision: replaced"],
    ];
    for (const [descriptor, count, why] of steps) {
      const call = generateWith(descriptor, "a red fox", { nav });
      await settle();
      assert.equal(workers.length, count, why);
      const live = workers.at(-1);
      assert.ok(isEngineWorker(live, "model-worker.js"));
      assert.ok(workers.slice(0, -1).every((w) => w.terminated), `${why}: the previous model is gone first`);
      result(live, live.posted.length - 1);
      await call;
    }
  });
});

test("the imagine page's variant-less Janus descriptor shares the worker of the registry build it would load", async () => {
  const adapterNav = (f16) => ({ gpu: { requestAdapter: async () => ({ features: new Set(f16 ? ["shader-f16"] : []) }) } });
  const legacy = { task: "text-to-image", id: JANUS.id, revision: JANUS.revision, download_mb: 1 };
  for (const [f16, same, other] of [[true, "f16", "f32"], [false, "f32", "f16"]]) {
    await withWorkers(async (workers) => {
      const nav = adapterNav(f16);
      const a = generateWith({ ...legacy, engine: "janus" }, "a red fox", { nav });
      await settle();
      result(workers[0]);
      await a;
      const b = generateWith({ ...JANUS, variant: same }, "a red fox", { nav });
      await settle();
      assert.equal(workers.length, 1, `the ${same} descriptor reuses the legacy load on this GPU`);
      result(workers[0], 1);
      await b;
      const c = generateWith({ ...JANUS, variant: other }, "a red fox", { nav });
      await settle();
      assert.equal(workers.length, 2, `the ${other} build is another model`);
      result(workers[1]);
      await c;
    });
  }
});

test("a fatal SD error retires its worker and the next generation gets a fresh one", async () => {
  await withWorkers(async (workers) => {
    const first = generateWith(SD, "a red fox", { nav });
    await settle();
    workers[0].emit({
      type: "error",
      id: workers[0].posted[0].id,
      error: "This browser doesn't have enough free storage to keep the image model (about 1.7 GB).",
      fatal: true,
    });
    await assert.rejects(first, /enough free storage/);
    assert.equal(workers[0].terminated, true);
    const second = generateWith(SD, "a red fox", { nav });
    await settle();
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
    await settle();
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
