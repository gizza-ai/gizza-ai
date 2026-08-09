import test from "node:test";
import assert from "node:assert/strict";

import {
  compatibleDevices,
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
