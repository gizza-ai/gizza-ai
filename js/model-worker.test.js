// The real model-worker.js, run in Node with a fake Transformers.js (resolved
// in place of ./transformers.min.js) and a fake WebGPU adapter.
import { test } from "node:test";
import assert from "node:assert/strict";
import { registerHooks } from "node:module";

import { IMAGE_MODELS, resolveVariant } from "../tools/generator/assets/runtime/image-models.js";
import { JANUS_DTYPES, janusLoadKey, janusVariant } from "../tools/generator/assets/runtime/janus-builds.js";

const FAKE = new URL("./fixtures/model-worker/fake-transformers.js", import.meta.url).href;
registerHooks({
  resolve(specifier, context, next) {
    if (specifier === "./transformers.min.js" && String(context.parentURL).endsWith("/model-worker.js")) {
      return { url: FAKE, shortCircuit: true };
    }
    return next(specifier, context);
  },
});

const listeners = [];
const posted = [];
const waiters = [];
let adapter = null;
globalThis.self = globalThis;
globalThis.addEventListener = (type, fn) => {
  if (type === "message") listeners.push(fn);
};
globalThis.postMessage = (message) => {
  posted.push(message);
  for (const w of [...waiters]) {
    if (w.pred(message)) {
      waiters.splice(waiters.indexOf(w), 1);
      w.resolve(message);
    }
  }
};
Object.defineProperty(globalThis, "navigator", {
  configurable: true,
  value: { gpu: { requestAdapter: async () => adapter } },
});

const { fake } = await import(FAKE);
await import("../tools/generator/assets/runtime/model-worker.js");

const send = (data) => listeners.forEach((fn) => fn({ data }));
const next = (pred) => {
  const hit = posted.find(pred);
  if (hit) return Promise.resolve(hit);
  return new Promise((resolve) => waiters.push({ pred, resolve }));
};
const final = (id) => next((m) => m.id === id && m.type !== "progress");
const tick = () => new Promise((r) => setTimeout(r, 0));
const gpu = (f16) => ({ features: new Set(f16 ? ["shader-f16"] : []), limits: { maxBufferSize: 2 ** 31 } });
const F16_INFO = { webgpu: true, shaderF16: true, maxBufferSize: 2 ** 31 };
const JANUS_ENTRY = IMAGE_MODELS.find((m) => m.engine === "janus");

let nextId = 1;
// Generates with `model`, finishes whatever loads it starts, and returns the
// reply plus the model load's options (null when no load started).
async function generate(model) {
  const id = nextId++;
  const before = fake.loads.length;
  send({ type: "generate", id, model, prompt: "a red fox" });
  await tick();
  const started = fake.loads.slice(before);
  for (const load of started) load.finish();
  const reply = await final(id);
  return { id, reply, model: started.find((l) => l.kind === "model")?.options ?? null };
}

test("Janus on an f16 device resolves f16 and the worker loads f16", async () => {
  adapter = gpu(true);
  const { name, descriptor } = resolveVariant(JANUS_ENTRY, F16_INFO);
  assert.equal(name, "f16");
  const { reply, model } = await generate(descriptor);
  assert.equal(reply.type, "result", JSON.stringify(reply));
  assert.deepEqual(model.dtype, JANUS_DTYPES.f16);
  assert.equal(model.revision, JANUS_ENTRY.revision);
});

test("an f32 variant loads f32 even on an f16 device", async () => {
  adapter = gpu(true);
  // A custom repo that only ships the f32 files: the user accepted .../f32.
  const descriptor = { ...resolveVariant(JANUS_ENTRY, F16_INFO).descriptor, id: "acme/janus-f32-only", variant: "f32" };
  const { reply, model } = await generate(descriptor);
  assert.equal(reply.type, "result", JSON.stringify(reply));
  assert.deepEqual(model.dtype, JANUS_DTYPES.f32);
});

test("the legacy imagine descriptor, with no variant, is still picked from the GPU", async () => {
  const legacy = { task: "text-to-image", id: "acme/janus-legacy", revision: "abc", download_mb: 1 };
  adapter = gpu(false);
  assert.deepEqual((await generate(legacy)).model.dtype, JANUS_DTYPES.f32);
  adapter = gpu(true);
  assert.deepEqual((await generate({ ...legacy, revision: "def" })).model.dtype, JANUS_DTYPES.f16);
});

test("an f16 variant on a GPU without shader-f16 is a readable error, not a load", async () => {
  adapter = gpu(false);
  const before = fake.loads.length;
  const { reply } = await generate({ ...resolveVariant(JANUS_ENTRY, F16_INFO).descriptor, id: "acme/janus-f16" });
  assert.equal(reply.type, "error");
  assert.match(reply.error, /shader-f16/);
  assert.equal(fake.loads.length, before, "nothing downloads");
});

test("loading is reported only while the weights load; a warm generation starts at generate 0", async () => {
  adapter = gpu(true);
  const descriptor = { ...resolveVariant(JANUS_ENTRY, F16_INFO).descriptor, id: "acme/janus-warm" };
  const cold = await generate(descriptor);
  const coldStages = posted.filter((m) => m.id === cold.id && m.type === "progress").map((m) => m.progress.stage);
  assert.equal(coldStages[0], "loading");
  const warm = await generate(descriptor);
  assert.equal(warm.reply.type, "result");
  assert.equal(warm.model, null, "the warm generation reuses the load");
  const warmStages = posted.filter((m) => m.id === warm.id && m.type === "progress").map((m) => m.progress);
  assert.ok(!warmStages.some((p) => p.stage === "loading" || p.stage === "download"));
  assert.deepEqual(warmStages[0], { stage: "generate", count: 0, total: 4 });
});

test("the load key and the variant pick are per build", () => {
  const base = { id: "a/b", revision: "r" };
  assert.notEqual(janusLoadKey({ ...base, variant: "f16" }), janusLoadKey({ ...base, variant: "f32" }));
  assert.notEqual(janusLoadKey({ ...base, variant: "f16" }), janusLoadKey(base));
  assert.throws(() => janusVariant({ ...base, variant: "int4" }, gpu(true)), /no "int4" build/);
});
