import test from "node:test";
import assert from "node:assert/strict";

import {
  compatibleDevices,
  generateImage,
  MODEL_FILE_LIMITS,
  ModelWorkerClient,
  progressLabel,
  validateModelFile,
  validateModelResult,
} from "../tools/generator/assets/runtime/tool-model.js";

test("compatibleDevices prefers WebGPU and keeps WASM fallback", () => {
  const model = { devices: ["webgpu", "wasm", "webgpu", "other"] };
  assert.deepEqual(compatibleDevices(model, { gpu: {} }), ["webgpu", "wasm"]);
  assert.deepEqual(compatibleDevices(model, {}), ["wasm"]);
  assert.deepEqual(compatibleDevices({}, {}), ["wasm"]);
});

test("progressLabel explains download, inference, and fallback states", () => {
  assert.equal(
    progressLabel({ stage: "download", progress: 0.42 }),
    "Downloading AI model… 42%",
  );
  assert.equal(
    progressLabel({ stage: "inference", device: "webgpu" }),
    "Processing locally with webgpu…",
  );
  assert.equal(
    progressLabel({ stage: "fallback", device: "wasm" }),
    "Trying wasm…",
  );
});

test("validateModelFile rejects non-images and oversized files", async () => {
  await assert.rejects(
    validateModelFile({ type: "text/plain", size: 10 }),
    /PNG, JPEG, or WebP/,
  );
  await assert.rejects(
    validateModelFile({ type: "image/svg+xml", size: 10 }),
    /PNG, JPEG, or WebP/,
  );
  await assert.rejects(
    validateModelFile({ type: "image/png", size: 21 * 1024 * 1024 }),
    /limit is 20 MB/,
  );
  assert.equal(
    await validateModelFile({ type: "image/png", size: 10 }),
    null,
    "Node has no createImageBitmap, so byte/type validation is still usable",
  );
});

test("validateModelFile enforces the exported MODEL_FILE_LIMITS", async () => {
  const { maxBytes, types } = MODEL_FILE_LIMITS;
  for (const type of types) {
    assert.equal(await validateModelFile({ type, size: maxBytes }), null, `${type} at the byte limit passes`);
  }
  await assert.rejects(validateModelFile({ type: types[0], size: maxBytes + 1 }), /too large/);
  assert.ok(Object.isFrozen(MODEL_FILE_LIMITS), "callers can't loosen the shared limits");
});

test("validateModelResult rejects empty cutouts and accepts a mixed alpha result", () => {
  const blob = new Blob(["png"], { type: "image/png" });
  assert.throws(
    () => validateModelResult({ blob, metrics: { foregroundRatio: 0, backgroundRatio: 1 } }),
    /No foreground subject was detected/,
  );
  assert.throws(
    () => validateModelResult({ blob, metrics: { foregroundRatio: 1, backgroundRatio: 0 } }),
    /could not be separated/,
  );
  assert.throws(
    () => validateModelResult({
      blob,
      metrics: { foregroundRatio: 0.4, backgroundRatio: 0.6, opaqueRatio: 0, transparentRatio: 0.5 },
    }),
    /foreground mask is too faint/,
  );
  assert.throws(
    () => validateModelResult({
      blob,
      metrics: { foregroundRatio: 0.4, backgroundRatio: 0.6, opaqueRatio: 0.5, transparentRatio: 0 },
    }),
    /background was not made transparent/,
  );
  const result = {
    blob,
    metrics: {
      foregroundRatio: 0.4,
      backgroundRatio: 0.6,
      opaqueRatio: 0.3,
      transparentRatio: 0.5,
    },
  };
  assert.equal(validateModelResult(result), result);
});

class FakeWorker {
  listeners = new Map();
  posted = [];
  terminated = false;

  addEventListener(type, fn) {
    this.listeners.set(type, fn);
  }

  postMessage(message) {
    this.posted.push(message);
  }

  emit(type, data) {
    this.listeners.get(type)?.({ data });
  }

  terminate() {
    this.terminated = true;
  }
}

test("ModelWorkerClient forwards progress and resolves a worker result", async () => {
  const worker = new FakeWorker();
  const client = new ModelWorkerClient(() => worker);
  const seen = [];
  const promise = client.run(
    { task: "u2net-background-removal", id: "BritishWerewolf/U-2-Netp", revision: "abc", devices: ["wasm"] },
    { type: "image/png", size: 1 },
    {},
    (progress) => seen.push(progress),
  );
  const id = worker.posted[0].id;
  worker.emit("message", { type: "progress", id, progress: { progress: 50 } });
  worker.emit("message", {
    type: "result",
    id,
    blob: new Blob(["png"], { type: "image/png" }),
    filename: "background-removed.png",
    backend: "wasm",
  });
  const result = await promise;
  assert.deepEqual(seen, [{ progress: 50 }]);
  assert.equal(result.backend, "wasm");
  assert.equal(result.blob.type, "image/png");
  client.dispose();
  assert.equal(worker.terminated, true);
});

test("ModelWorkerClient rejects worker-reported failures", async () => {
  const worker = new FakeWorker();
  const client = new ModelWorkerClient(() => worker);
  const promise = client.run(
    { task: "u2net-background-removal", id: "BritishWerewolf/U-2-Netp", revision: "abc", devices: ["wasm"] },
    { type: "image/png", size: 1 },
  );
  const id = worker.posted[0].id;
  worker.emit("message", { type: "error", id, error: "model failed" });
  await assert.rejects(promise, /model failed/);
  client.dispose();
});

const JANUS = { task: "text-to-image", id: "onnx-community/Janus-Pro-1B-ONNX", revision: "abc", devices: ["webgpu"] };

test("progressLabel reports image-token generation progress", () => {
  assert.equal(progressLabel({ stage: "generate", count: 144, total: 576 }), "Generating image… 25%");
});

test("ModelWorkerClient.generate posts the prompt and resolves the generated image", async () => {
  const worker = new FakeWorker();
  const client = new ModelWorkerClient(() => worker);
  const seen = [];
  const promise = client.generate(JANUS, "a red fox", { onProgress: (p) => seen.push(p) });
  const posted = worker.posted[0];
  assert.equal(posted.type, "generate");
  assert.equal(posted.prompt, "a red fox");
  assert.equal(posted.model.id, JANUS.id);
  worker.emit("message", { type: "progress", id: posted.id, progress: { stage: "generate", count: 1, total: 576 } });
  worker.emit("message", { type: "result", id: posted.id, blob: new Blob(["png"], { type: "image/png" }), filename: "generated.png", backend: "webgpu" });
  const result = await promise;
  assert.equal(result.filename, "generated.png");
  assert.deepEqual(seen, [{ stage: "generate", count: 1, total: 576 }]);
  client.dispose();
});

test("aborting a generation asks the worker to cancel and rejects with AbortError", async () => {
  const worker = new FakeWorker();
  const client = new ModelWorkerClient(() => worker);
  const controller = new AbortController();
  const promise = client.generate(JANUS, "a red fox", { signal: controller.signal });
  const id = worker.posted[0].id;
  controller.abort();
  assert.deepEqual(worker.posted[1], { type: "cancel", id });
  worker.emit("message", { type: "cancelled", id });
  await assert.rejects(promise, (error) => error.name === "AbortError");
  client.dispose();
});

test("an already-aborted signal never starts a generation", async () => {
  const worker = new FakeWorker();
  const client = new ModelWorkerClient(() => worker);
  const controller = new AbortController();
  controller.abort();
  await assert.rejects(client.generate(JANUS, "x", { signal: controller.signal }), (error) => error.name === "AbortError");
  assert.equal(worker.posted.length, 0);
  client.dispose();
});

test("generateImage needs WebGPU, a pinned text-to-image model and a prompt", async () => {
  await assert.rejects(generateImage(JANUS, "a fox", { nav: {} }), /needs WebGPU/);
  await assert.rejects(generateImage({ ...JANUS, revision: "" }, "a fox", { nav: { gpu: {} } }), /pinned model/);
  await assert.rejects(generateImage({ ...JANUS, task: "background-removal" }, "a fox", { nav: { gpu: {} } }), /text-to-image/);
  await assert.rejects(generateImage(JANUS, "   ", { nav: { gpu: {} } }), /Describe the image/);
});
