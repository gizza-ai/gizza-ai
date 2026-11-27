// Run a published tool on one input file — the single entry point shared by
// the tool page (tool.js) and any page that drives a tool by its spec (e.g. an
// editor applying a tool to its current image).
//
// `page` is the tool's page spec: window.GIZZA_TOOL on the tool page, or the
// `page` section of /tools/<slug>/tool.json elsewhere — same shape
// ({runtime, export, module, format, model, inputs[]}). `mod` is the tool's
// initialised web-wasm module (ffmpeg only: model and wasm-image tools run in
// workers that load what they need). `values` maps
// field name → raw string value; coercion happens here (tool-fields.js).
//
// Resolves {blob, outName, mime, backend?}; rejects with an Error whose message
// is fit to show the user. `deps.signal` (an AbortSignal) cancels a superseded
// or cancelled run, which rejects with an AbortError that callers drop rather
// than show: a text-to-image run is handed the signal, other model tools check
// it after input validation (before inference starts), and a wasm-image job
// that hasn't finished is dropped. Runtime modules are imported lazily so a page only
// fetches the runtime it uses.

import { coerceField, fieldArgs } from "./tool-fields.js";

export async function runTool(page, mod, file, values, deps = {}) {
  const read = (input) => (values && values[input.name] != null ? values[input.name] : "");
  switch (page && page.runtime) {
    case "ffmpeg": {
      if (typeof deps.execBytes !== "function") throw new Error("ffmpeg is not available on this page.");
      const { runFfmpeg } = await import("./tool-ffmpeg.js");
      const r = await runFfmpeg(page, mod, deps.execBytes, file, fieldArgs(page.inputs, read));
      if (!r.ok) throw new Error(r.error);
      return { blob: r.blob, outName: r.outName, mime: r.mime };
    }
    case "model": {
      const model = await import("./tool-model.js");
      // Text-to-image tools take no file: their `prompt` field is the input.
      if (page.model && page.model.task === "text-to-image") {
        const prompt = read(page.inputs.find((i) => i.name === "prompt") || { name: "prompt" });
        const result = await (deps.generateImage || model.generateImage)(page.model, prompt, {
          onProgress: deps.onProgress,
          signal: deps.signal,
        });
        return {
          blob: result.blob,
          outName: result.filename || "generated.png",
          mime: result.blob.type || "image/png",
          backend: result.backend,
        };
      }
      const run = deps.runModel || model.runModel;
      await (deps.validateFile || model.validateModelFile)(file);
      if (deps.signal) deps.signal.throwIfAborted();
      const fields = Object.fromEntries(
        page.inputs.filter((i) => i.source === "field").map((i) => [i.name, read(i)]),
      );
      const result = model.validateModelResult(await run(page.model, file, fields, deps.onProgress));
      return {
        blob: result.blob,
        outName: result.filename || `${page.slug || "result"}.png`,
        mime: result.blob.type || "image/png",
        backend: result.backend,
      };
    }
    case "wasm-image": {
      const [{ paramsFor, sharedWasmImageClient }, { mimeForOutput }] = await Promise.all([
        import("./tool-wasm-image.js"),
        import("./tool-mime.js"),
      ]);
      // The web module URL is relative to the tool's own directory: the page's
      // location on the tool page, `deps.baseUrl` (/tools/<slug>/) elsewhere.
      const moduleUrl = new URL(page.module, deps.baseUrl || globalThis.location.href).href;
      const client = deps.wasmImageClient || sharedWasmImageClient();
      const bytes = new Uint8Array(await file.arrayBuffer());
      const params = paramsFor(page.inputs, read, coerceField);
      const r = await client.run(moduleUrl, page.export, params, bytes, { signal: deps.signal });
      const mime = mimeForOutput(r.outName);
      return { blob: new Blob([r.bytes], { type: mime }), outName: r.outName, mime };
    }
    default:
      throw new Error(`unsupported runtime "${page && page.runtime}"`);
  }
}
