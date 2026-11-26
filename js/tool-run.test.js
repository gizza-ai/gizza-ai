import { test } from "node:test";
import assert from "node:assert/strict";
import { runTool } from "../tools/generator/assets/runtime/tool-run.js";

const file = { name: "photo.png", type: "image/png", size: 2, arrayBuffer: async () => new Uint8Array([7, 7]).buffer };

const resizePage = {
  runtime: "ffmpeg",
  export: "build_argv",
  inputs: [
    { name: "image", source: "file" },
    { name: "width", source: "field", numeric: true },
    { name: "fit", source: "field" },
  ],
};

test("ffmpeg runtime passes coerced positional args in declared order", async () => {
  let argvArgs;
  const mod = { build_argv: (...a) => { argvArgs = a; return { argv: ["x"], out_name: "out.png" }; } };
  const execBytes = async () => ({ exit_code: 0, output: new Uint8Array([1]), log: "" });
  const r = await runTool(resizePage, mod, file, { width: "320", fit: "cover" }, { execBytes });
  assert.deepEqual(argvArgs, [320, "cover", "in.png"]);
  assert.equal(r.outName, "out.png");
  assert.equal(r.mime, "image/png");
  assert.equal(r.blob.size, 1);
});

test("missing values pass as empty strings so the wasm applies its defaults", async () => {
  let argvArgs;
  const mod = { build_argv: (...a) => { argvArgs = a; return { argv: [], out_name: "out.png" }; } };
  const execBytes = async () => ({ exit_code: 0, output: new Uint8Array([1]), log: "" });
  await runTool(resizePage, mod, file, {}, { execBytes });
  assert.deepEqual(argvArgs, ["", "", "in.png"]);
});

test("a failed ffmpeg run rejects with the log line", async () => {
  const mod = { build_argv: () => ({ argv: [], out_name: "out.png" }) };
  const execBytes = async () => ({ exit_code: 1, output: null, log: "Conversion failed!\n" });
  await assert.rejects(runTool(resizePage, mod, file, {}, { execBytes }), /Conversion failed!/);
});

test("model runtime validates, runs and returns the blob", async () => {
  const page = { runtime: "model", slug: "bg", model: { task: "t" }, inputs: [{ name: "image", source: "file" }] };
  const blob = new Blob([new Uint8Array([1])], { type: "image/png" });
  const r = await runTool(page, null, file, {}, {
    validateFile: async () => null,
    runModel: async (model, f) => { assert.equal(f, file); return { blob, filename: "cut.png", backend: "wasm" }; },
  });
  assert.equal(r.outName, "cut.png");
  assert.equal(r.backend, "wasm");
});

test("model runtime aborted after validation never calls runModel", async () => {
  const page = { runtime: "model", slug: "bg", model: { task: "t" }, inputs: [{ name: "image", source: "file" }] };
  const controller = new AbortController();
  let ran = false;
  await assert.rejects(
    runTool(page, null, file, {}, {
      signal: controller.signal,
      // A newer run supersedes this one while its input is being validated.
      validateFile: async () => { controller.abort(); return null; },
      runModel: async () => { ran = true; return {}; },
    }),
    { name: "AbortError" },
  );
  assert.equal(ran, false);
});

test("unknown runtime is rejected", async () => {
  await assert.rejects(runTool({ runtime: "wat", inputs: [] }, null, file, {}), /unsupported runtime "wat"/);
});
