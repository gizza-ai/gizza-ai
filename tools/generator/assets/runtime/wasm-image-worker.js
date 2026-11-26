// Runs `wasm-image` tool exports off the main thread so decoding/encoding a
// large image never freezes the page. One worker serves every tool module a
// page uses; each module is imported and initialised once.
//
// Request:  { type: "run", id, moduleUrl, export, params, bytes }
// Response: { type: "result", id, bytes, outName } | { type: "error", id, error, trapped }
//
// `trapped` marks a wasm trap (WebAssembly.RuntimeError: a panic or an
// out-of-bounds access). The trapped instance's memory and allocator are left
// in an undefined state, and a wasm-pack module never re-initialises once
// loaded, so the instance cannot be dropped from inside this worker: the
// client retires the whole worker and continues on a fresh one.

const modules = new Map();

function loadModule(url) {
  let promise = modules.get(url);
  if (!promise) {
    promise = import(url).then(async (mod) => {
      await mod.default(); // wasm-pack --target web init
      return mod;
    });
    modules.set(url, promise);
    promise.catch(() => modules.delete(url));
  }
  return promise;
}

function message(error) {
  if (typeof error === "string") return error;
  if (error && typeof error.message === "string") return error.message;
  return "The tool failed to process this image.";
}

self.addEventListener("message", async (event) => {
  const req = event.data || {};
  if (req.type !== "run") return;
  try {
    const mod = await loadModule(req.moduleUrl);
    const out = mod[req.export](req.params, req.bytes);
    const bytes = out.bytes;
    self.postMessage({ type: "result", id: req.id, bytes, outName: out.out_name }, [bytes.buffer]);
  } catch (error) {
    const trapped = error instanceof WebAssembly.RuntimeError;
    self.postMessage({ type: "error", id: req.id, error: message(error), trapped });
  }
});
