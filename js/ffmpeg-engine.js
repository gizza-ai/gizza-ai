// ffmpeg-engine.js — gizza-ai chat ffmpeg page-side engine.
//
// Runs in the window, where dynamic import() and Worker — both forbidden in a
// ServiceWorkerGlobalScope — are allowed. Listens for `ffmpeg-exec-request`
// messages the SW posts (see js/ffmpeg-bridge.js), runs the real ffmpeg via the
// shared js/ffmpeg.js, and posts an `ffmpeg-exec-response` back on the global SW
// channel (reg.active.postMessage) — the same channel webllm-engine.js uses.

// Lazy + injectable so node unit tests can drive runFfmpegRequest without a real
// /ffmpeg.js (which dynamically imports the @ffmpeg CDN bundle).
async function defaultExec(argsJson, inputsJson, outputName) {
  const mod = await import('/ffmpeg.js');
  return mod.ffmpegExec(argsJson, inputsJson, outputName);
}

// Run one ffmpeg request and return a BridgeResponse-shaped result. Never
// throws: page-side failures are encoded as exit_code -1 + log so the Rust
// BrowserFfmpegService (which never handles a rejected promise) stays unchanged.
export async function runFfmpegRequest(msg, exec = defaultExec) {
  try {
    const r = await exec(msg.args_json, msg.inputs_json, msg.output_name);
    return { exit_code: r.exit_code, output_b64: r.output_b64, log: r.log };
  } catch (e) {
    return { exit_code: -1, output_b64: '', log: String(e) };
  }
}

async function swPost(payload) {
  const reg = await navigator.serviceWorker.ready;
  reg.active?.postMessage(payload);
}

// Register the page-side listener only in a real browser (skipped under node,
// where navigator is undefined).
if (typeof navigator !== 'undefined' && navigator.serviceWorker) {
  navigator.serviceWorker.addEventListener('message', async (event) => {
    const msg = event.data;
    if (!msg || msg.type !== 'ffmpeg-exec-request') return;
    const resp = await runFfmpegRequest(msg);
    await swPost({ type: 'ffmpeg-exec-response', id: msg.id, ...resp });
  });
  console.log('ffmpeg-engine.js loaded');
}
