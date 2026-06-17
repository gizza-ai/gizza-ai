// ffmpeg-bridge.js — gizza-ai chat ffmpeg SW-side bridge.
//
// Runs inside the Service Worker (imported as a wasm-bindgen snippet by
// BrowserFfmpegService). ffmpeg can't run in a ServiceWorkerGlobalScope
// (import()/Worker forbidden), so this posts an `ffmpeg-exec-request` to a
// window client and awaits the matching `ffmpeg-exec-response`. The page
// (js/ffmpeg-engine.js) replies on the global SW channel, which fans out to
// every SW `message` listener — including the one this module registers below.
// The framework sw.js central handler ignores the unmatched `ffmpeg-*` type.
//
// `ffmpegExec` keeps the exact name + (argsJson, inputsJson, outputName)
// signature the Rust binding expects, and ALWAYS RESOLVES with a
// { exit_code, output_b64, log } object (errors encoded as exit_code -1) so the
// Rust side stays unchanged except its module path.

const pending = new Map(); // id -> { resolve }
let _counter = 0;

export function makeId() {
  return `ffmpeg-${++_counter}`;
}

export function registerPending(id) {
  return new Promise((resolve) => { pending.set(id, { resolve }); });
}

// Resolve the pending request matching an `ffmpeg-exec-response`. Returns true
// iff this module consumed the message; foreign types and unknown ids are
// ignored so the central sw.js routing is never disturbed.
export function completeBridgeMessage(msg) {
  if (!msg || msg.type !== 'ffmpeg-exec-response') return false;
  const entry = pending.get(msg.id);
  if (!entry) return false;
  pending.delete(msg.id);
  entry.resolve({ exit_code: msg.exit_code, output_b64: msg.output_b64, log: msg.log });
  return true;
}

async function postToWindowClient(msg) {
  const clients = await self.clients.matchAll({ type: 'window' });
  if (!clients.length) {
    // No page to run ffmpeg — fail the pending request through the same path
    // (any window client is fine; the id correlates the reply).
    completeBridgeMessage({
      type: 'ffmpeg-exec-response',
      id: msg.id,
      exit_code: -1,
      output_b64: '',
      log: 'no window client available to run ffmpeg',
    });
    return;
  }
  clients[0].postMessage(msg);
}

// Called by Rust BrowserFfmpegService. `post` is injectable for tests; in the
// SW it defaults to posting to a window client.
export async function ffmpegExec(argsJson, inputsJson, outputName, post = postToWindowClient) {
  const id = makeId();
  const result = registerPending(id);
  await post({
    type: 'ffmpeg-exec-request',
    id,
    args_json: argsJson,
    inputs_json: inputsJson,
    output_name: outputName,
  });
  return result;
}

// Register our own SW message listener (skipped under node, where self is
// undefined). Multiple message listeners coexist; sw.js's central handler
// ignores `ffmpeg-exec-response`, so this is the only consumer.
if (typeof self !== 'undefined' && typeof self.addEventListener === 'function') {
  self.addEventListener('message', (event) => { completeBridgeMessage(event?.data); });
}
