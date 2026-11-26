import { test } from "node:test";
import assert from "node:assert/strict";
import { WasmImageClient } from "../tools/generator/assets/runtime/tool-wasm-image.js";

function fakeWorker() {
  const listeners = {};
  return {
    posted: [],
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

test("WasmImageClient rejects on a worker error message and on a crashed worker", async () => {
  const worker = fakeWorker();
  const client = new WasmImageClient(() => worker);
  const first = client.run("u", "run", {}, new Uint8Array([1]));
  worker.emit("message", { type: "error", id: 1, error: "bad image" });
  await assert.rejects(first, /bad image/);
  const second = client.run("u", "run", {}, new Uint8Array([1]));
  worker.emit("error", { message: "boom" });
  await assert.rejects(second, /boom/);
});
