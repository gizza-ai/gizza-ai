// Client for the `wasm-image` runtime: image bytes in → image bytes out through
// a tool's web export `run(params, input)`, executed in wasm-image-worker.js.

// The params object for `run`: field values keyed by name, typed by their
// control (numbers via coerceField, checkboxes as booleans), with blank fields
// omitted so the block core's serde defaults apply.
export function paramsFor(inputs, read, coerce) {
  const params = {};
  for (const input of inputs) {
    if (input.source !== "field") continue;
    const value = coerce(input, read(input));
    if (value === "") continue;
    params[input.name] =
      input.control && input.control.kind === "checkbox" ? value === "true" : value;
  }
  return params;
}

export class WasmImageClient {
  constructor(workerFactory = () => new Worker(new URL("./wasm-image-worker.js", import.meta.url), { type: "module" })) {
    this.worker = workerFactory();
    this.nextId = 1;
    this.pending = new Map();
    this.worker.addEventListener("message", (event) => {
      const msg = event.data || {};
      const pending = this.pending.get(msg.id);
      if (!pending) return;
      this.pending.delete(msg.id);
      if (msg.type === "result") pending.resolve({ bytes: msg.bytes, outName: msg.outName });
      else pending.reject(new Error(msg.error || "The tool failed to process this image."));
    });
    this.worker.addEventListener("error", (event) => {
      const error = new Error((event && event.message) || "The image worker failed.");
      for (const { reject } of this.pending.values()) reject(error);
      this.pending.clear();
    });
  }

  run(moduleUrl, exportName, params, bytes) {
    const id = this.nextId++;
    return new Promise((resolve, reject) => {
      this.pending.set(id, { resolve, reject });
      this.worker.postMessage({ type: "run", id, moduleUrl, export: exportName, params, bytes }, [bytes.buffer]);
    });
  }
}

let shared = null;
export function sharedWasmImageClient() {
  if (!shared) shared = new WasmImageClient();
  return shared;
}
