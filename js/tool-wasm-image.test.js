import { test } from "node:test";
import assert from "node:assert/strict";
import { WasmImageClient, sharedWasmImageClient } from "../tools/generator/assets/runtime/tool-wasm-image.js";

function fakeWorker() {
  const listeners = {};
  return {
    posted: [],
    terminated: false,
    terminate() { this.terminated = true; },
    addEventListener(type, fn) { listeners[type] = fn; },
    postMessage(msg, transfer) { this.posted.push({ msg, transfer }); },
    emit(type, data) { listeners[type](type === "message" ? { data } : data); },
  };
}

test("WasmImageClient transfers the bytes and resolves the matching result", async () => {
  const worker = fakeWorker();
  const client = new WasmImageClient(() => worker);
  const bytes = new Uint8Array([1, 2]);
  const pending = client.run("https://x/m.js", "run", { a: 1 }, bytes);
  const { msg, transfer } = worker.posted[0];
  assert.deepEqual({ ...msg, bytes: undefined }, { type: "run", id: 1, moduleUrl: "https://x/m.js", export: "run", params: { a: 1 }, bytes: undefined });
  assert.equal(transfer[0], bytes.buffer);
  worker.emit("message", { type: "result", id: 1, bytes: new Uint8Array([9]), outName: "out.png" });
  const r = await pending;
  assert.equal(r.outName, "out.png");
});

const ok = (id) => ({ type: "result", id, bytes: new Uint8Array([id]), outName: "out.png" });

test("WasmImageClient rejects a tool error and keeps serving on the same worker", async () => {
  const worker = fakeWorker();
  const client = new WasmImageClient(() => worker);
  const first = client.run("u", "run", {}, new Uint8Array([1]));
  worker.emit("message", { type: "error", id: 1, error: "bad image" });
  await assert.rejects(first, /bad image/);
  const second = client.run("u", "run", {}, new Uint8Array([1]));
  worker.emit("message", ok(2));
  assert.equal((await second).outName, "out.png");
  assert.equal(worker.terminated, false);
});

test("WasmImageClient posts one job at a time", async () => {
  const worker = fakeWorker();
  const client = new WasmImageClient(() => worker);
  const a = client.run("u", "run", {}, new Uint8Array([1]));
  const b = client.run("u", "run", {}, new Uint8Array([2]));
  assert.equal(worker.posted.length, 1);
  worker.emit("message", ok(1));
  await a;
  assert.equal(worker.posted.length, 2);
  worker.emit("message", ok(2));
  await b;
});

test("an aborted queued job rejects with AbortError and is never sent", async () => {
  const worker = fakeWorker();
  const client = new WasmImageClient(() => worker);
  const first = client.run("u", "run", {}, new Uint8Array([1]));
  const stale = new AbortController();
  const second = client.run("u", "run", {}, new Uint8Array([2]), { signal: stale.signal });
  const third = client.run("u", "run", {}, new Uint8Array([3]));
  stale.abort();
  await assert.rejects(second, { name: "AbortError" });
  worker.emit("message", ok(1));
  await first;
  assert.deepEqual(worker.posted.map((p) => p.msg.id), [1, 3]);
  worker.emit("message", ok(3));
  await third;
});

test("an aborted in-flight job rejects at once and its late result is dropped", async () => {
  const worker = fakeWorker();
  const client = new WasmImageClient(() => worker);
  const ctl = new AbortController();
  const first = client.run("u", "run", {}, new Uint8Array([1]), { signal: ctl.signal });
  const second = client.run("u", "run", {}, new Uint8Array([2]));
  ctl.abort();
  await assert.rejects(first, { name: "AbortError" });
  assert.equal(worker.posted.length, 1); // the worker is still busy with job 1
  worker.emit("message", ok(1));
  assert.equal(worker.posted.length, 2);
  worker.emit("message", ok(2));
  assert.equal((await second).outName, "out.png");
});

test("an already-aborted signal rejects without queueing", async () => {
  const worker = fakeWorker();
  const client = new WasmImageClient(() => worker);
  await assert.rejects(
    client.run("u", "run", {}, new Uint8Array([1]), { signal: AbortSignal.abort() }),
    { name: "AbortError" },
  );
  assert.equal(worker.posted.length, 0);
});

test("a wasm trap retires the worker and later jobs run on a fresh one", async () => {
  const workers = [];
  const client = new WasmImageClient(() => (workers.push(fakeWorker()), workers.at(-1)));
  const first = client.run("u", "run", {}, new Uint8Array([1]));
  const second = client.run("u", "run", {}, new Uint8Array([2]));
  workers[0].emit("message", { type: "error", id: 1, error: "unreachable", trapped: true });
  await assert.rejects(first, /unreachable/);
  assert.equal(workers[0].terminated, true);
  assert.equal(workers.length, 2);
  assert.deepEqual(workers[1].posted.map((p) => p.msg.id), [2]);
  workers[1].emit("message", ok(2));
  assert.equal((await second).outName, "out.png");
});

test("a crashed worker rejects every job, terminates, and the shared client is replaced", async () => {
  const workers = [];
  const factory = () => (workers.push(fakeWorker()), workers.at(-1));
  const client = sharedWasmImageClient(factory);
  assert.equal(sharedWasmImageClient(factory), client);
  const inFlight = client.run("u", "run", {}, new Uint8Array([1]));
  const queued = client.run("u", "run", {}, new Uint8Array([2]));
  workers[0].emit("error", { message: "boom" });
  await assert.rejects(inFlight, /boom/);
  await assert.rejects(queued, /boom/);
  assert.equal(workers[0].terminated, true);
  await assert.rejects(client.run("u", "run", {}, new Uint8Array([3])), /boom/); // never hangs
  const fresh = sharedWasmImageClient(factory);
  assert.notEqual(fresh, client);
  const r = fresh.run("u", "run", {}, new Uint8Array([4]));
  workers[1].emit("message", ok(fresh.nextId - 1));
  assert.equal((await r).outName, "out.png");
});

test("a postMessage failure rejects that job and the queue moves on", async () => {
  const worker = fakeWorker();
  let throwOnce = true;
  const post = worker.postMessage;
  worker.postMessage = function (msg, transfer) {
    if (throwOnce) { throwOnce = false; throw new Error("DataCloneError"); }
    return post.call(this, msg, transfer);
  };
  const client = new WasmImageClient(() => worker);
  const bad = client.run("u", "run", {}, new Uint8Array([1]));
  const good = client.run("u", "run", {}, new Uint8Array([2]));
  await assert.rejects(bad, /DataCloneError/);
  assert.deepEqual(worker.posted.map((p) => p.msg.id), [2]);
  worker.emit("message", ok(2));
  await good;
});

test("the worker flags a wasm trap as trapped and a tool error as not", async () => {
  let onMessage;
  const replies = [];
  globalThis.self = {
    addEventListener: (type, fn) => { if (type === "message") onMessage = fn; },
    postMessage: (msg) => replies.push(msg),
  };
  try {
    await import("../tools/generator/assets/runtime/wasm-image-worker.js");
    await runWorkerCases(onMessage, replies);
  } finally {
    delete globalThis.self;
  }
});

async function runWorkerCases(onMessage, replies) {
  const source = `export default async () => {};
    export function trap() { throw new WebAssembly.RuntimeError("unreachable"); }
    export function reject() { throw "could not decode image"; }`;
  const moduleUrl = "data:text/javascript," + encodeURIComponent(source);
  await onMessage({ data: { type: "run", id: 1, moduleUrl, export: "trap", params: {}, bytes: new Uint8Array() } });
  await onMessage({ data: { type: "run", id: 2, moduleUrl, export: "reject", params: {}, bytes: new Uint8Array() } });
  assert.deepEqual(replies, [
    { type: "error", id: 1, error: "unreachable", trapped: true },
    { type: "error", id: 2, error: "could not decode image", trapped: false },
  ]);
}
