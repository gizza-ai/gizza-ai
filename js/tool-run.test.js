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

test("text-to-image models generate from the prompt field without a file", async () => {
  const page = {
    runtime: "model",
    model: { task: "text-to-image", id: "m", revision: "r" },
    inputs: [{ name: "prompt", source: "field" }],
  };
  const signal = new AbortController().signal;
  let call;
  const r = await runTool(page, null, null, { prompt: "a lighthouse at dusk" }, {
    signal,
    generateImage: async (model, prompt, opts) => {
      call = { model, prompt, signal: opts.signal };
      return { blob: new Blob([new Uint8Array([1])], { type: "image/png" }), filename: "generated.png", backend: "webgpu" };
    },
  });
  assert.equal(call.prompt, "a lighthouse at dusk");
  assert.equal(call.model, page.model);
  assert.equal(call.signal, signal);
  assert.equal(r.outName, "generated.png");
  assert.equal(r.backend, "webgpu");
});

test("wasm-image runtime sends a typed params object and the bytes, resolving the module against baseUrl", async () => {
  const page = {
    runtime: "wasm-image",
    export: "run",
    module: "./gizza_ai_image_brightness_contrast_web.js",
    inputs: [
      { name: "image", source: "file" },
      { name: "brightness", source: "field", numeric: true, control: { kind: "slider" } },
      { name: "contrast", source: "field", numeric: true, control: { kind: "slider" } },
      { name: "keep_alpha", source: "field", control: { kind: "checkbox" } },
    ],
  };
  let call;
  const wasmImageClient = {
    run: async (moduleUrl, exportName, params, bytes, opts) => {
      call = { moduleUrl, exportName, params, bytes: Array.from(bytes), signal: opts.signal };
      return { bytes: new Uint8Array([4, 5]), outName: "adjusted.png" };
    },
  };
  const signal = new AbortController().signal;
  const r = await runTool(page, null, file, { brightness: "20", contrast: "", keep_alpha: "true" }, {
    baseUrl: "https://example.test/tools/image-brightness-contrast/",
    wasmImageClient,
    signal,
  });
  assert.deepEqual(call, {
    moduleUrl: "https://example.test/tools/image-brightness-contrast/gizza_ai_image_brightness_contrast_web.js",
    exportName: "run",
    params: { brightness: 20, keep_alpha: true },
    bytes: [7, 7],
    signal,
  });
  assert.equal(r.outName, "adjusted.png");
  assert.equal(r.mime, "image/png");
  assert.equal(r.blob.size, 2);
});

test("wasm-image errors reject with the tool's message", async () => {
  const page = { runtime: "wasm-image", export: "run", module: "./m.js", inputs: [] };
  const wasmImageClient = { run: async () => { throw new Error("could not decode image"); } };
  await assert.rejects(
    runTool(page, null, file, {}, { baseUrl: "https://example.test/tools/x/", wasmImageClient }),
    /could not decode image/,
  );
});
