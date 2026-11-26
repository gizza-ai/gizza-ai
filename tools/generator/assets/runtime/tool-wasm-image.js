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

function abortError() {
  return new DOMException("The image job was cancelled.", "AbortError");
}

const defaultWorker = () =>
  new Worker(new URL("./wasm-image-worker.js", import.meta.url), { type: "module" });

// Jobs run one at a time: the client holds the queue and posts the next job
// only when the worker has answered the previous one. A wasm call blocks the
// worker, so this costs nothing — and it means an aborted job that has not
// started is simply dropped here instead of being ground through by the
// worker (a slider drag never leaves a backlog of stale full-image runs).
//
// A job that traps the wasm (`trapped` error) rejects with the tool's message
// and retires that worker: the queue continues on a fresh one, so no later
// job runs on the poisoned instance. A crashed worker (`error` event) breaks
// the client for good: every queued and in-flight job rejects, the worker is
// terminated, and `broken` tells sharedWasmImageClient() to start a new one.
export class WasmImageClient {
  constructor(workerFactory = defaultWorker) {
    this.workerFactory = workerFactory;
    this.nextId = 1;
    this.queue = [];
    this.active = null;
    this.broken = null;
    this.spawn();
  }

  spawn() {
    const worker = this.workerFactory();
    this.worker = worker;
    worker.addEventListener("message", (event) => {
      const msg = event.data || {};
      const job = this.active;
      if (worker !== this.worker || !job || job.id !== msg.id) return;
      this.active = null;
      if (msg.type === "result") {
        job.resolve({ bytes: msg.bytes, outName: msg.outName });
      } else {
        job.reject(new Error(msg.error || "The tool failed to process this image."));
        if (msg.trapped) {
          worker.terminate();
          this.spawn();
        }
      }
      this.pump();
    });
    worker.addEventListener("error", (event) => {
      if (worker !== this.worker) return;
      this.fail(new Error((event && event.message) || "The image worker failed."));
    });
  }

  fail(error) {
    if (this.broken) return;
    this.broken = error;
    const jobs = this.active ? [this.active, ...this.queue] : this.queue;
    this.active = null;
    this.queue = [];
    for (const job of jobs) job.reject(error);
    this.worker.terminate();
  }

  // Resolves {bytes, outName}. `signal` (an AbortSignal) cancels the job: it
  // rejects with an AbortError at once; a queued job is never sent, an
  // in-flight one finishes in the worker and its result is discarded.
  run(moduleUrl, exportName, params, bytes, { signal } = {}) {
    if (this.broken) return Promise.reject(this.broken);
    if (signal && signal.aborted) return Promise.reject(abortError());
    return new Promise((resolve, reject) => {
      const job = { id: this.nextId++, moduleUrl, exportName, params, bytes };
      const settle = (fn) => (value) => {
        if (signal) signal.removeEventListener("abort", onAbort);
        fn(value);
      };
      job.resolve = settle(resolve);
      job.reject = settle(reject);
      const onAbort = () => {
        const i = this.queue.indexOf(job);
        if (i >= 0) this.queue.splice(i, 1);
        job.resolve = job.reject = () => {}; // an in-flight result is dropped
        if (signal) signal.removeEventListener("abort", onAbort);
        reject(abortError());
      };
      if (signal) signal.addEventListener("abort", onAbort);
      this.queue.push(job);
      this.pump();
    });
  }

  pump() {
    if (this.active || this.broken) return;
    const job = this.queue.shift();
    if (!job) return;
    this.active = job;
    const { id, moduleUrl, exportName, params, bytes } = job;
    try {
      this.worker.postMessage({ type: "run", id, moduleUrl, export: exportName, params, bytes }, [bytes.buffer]);
    } catch (error) {
      // e.g. an un-cloneable params value or an already-detached buffer.
      this.active = null;
      job.reject(error instanceof Error ? error : new Error(String(error)));
      this.pump();
    }
  }
}

let shared = null;
export function sharedWasmImageClient(workerFactory = defaultWorker) {
  if (!shared || shared.broken) shared = new WasmImageClient(workerFactory);
  return shared;
}
