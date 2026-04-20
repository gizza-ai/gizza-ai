// sw-llm-bridge.js — SW-context wrapper for gizza's local-LLM chat_stream.
//
// Loaded by the wasm-bindgen glue as a snippet at Service-Worker import
// time (via agent.rs's `#[wasm_bindgen(module = "/site/sw-llm-bridge.js")]`).
// Does three things:
//
// 1. Registers a `message` listener on `self` so the SW receives SSE stream
//    chunks (`local-llm-stream-event`) and one-shot responses
//    (`local-llm-response`) back from the main-thread `ai-bridge.js` client.
//
// 2. Exports `localLlmChatStream(bodyJson)` — called synchronously from the
//    agent block's Rust code. It postMessages the request to a main-thread
//    client, buffers the SSE frames it sees come back on the `message`
//    listener, and resolves with a `Uint8Array` of the accumulated frames
//    when the stream closes.
//
// This file must NOT `import` anything from a third-party CDN (e.g. WebLLM
// from jsdelivr). The SW module graph has to fully resolve at install time;
// a CDN import at that point is a cross-origin fetch that blocks SW
// startup. WebLLM itself lives in main-thread `ai-bridge.js` where CDN
// imports are fine.

// Per-stream state keyed by the monotonically-increasing request id we
// send to the client.
const pendingStreams = new Map(); // id -> { chunks: Uint8Array[], resolve, reject, closed, timeoutId }

let requestId = 0;

self.addEventListener('message', (event) => {
    const msg = event.data;
    if (!msg) return;

    if (msg.type === 'local-llm-stream-event') {
        const stream = pendingStreams.get(msg.id);
        if (!stream || stream.closed) return;
        const { type, ...data } = msg.event;
        const sseFrame = `event: ${type}\ndata: ${JSON.stringify(data)}\n\n`;
        stream.chunks.push(new TextEncoder().encode(sseFrame));
        if (type === 'done') {
            stream.closed = true;
            clearTimeout(stream.timeoutId);
            pendingStreams.delete(msg.id);
            stream.resolve(concatChunks(stream.chunks));
        }
        return;
    }

    if (msg.type === 'local-llm-response') {
        // Error-teardown path for streaming requests (the main thread uses
        // `local-llm-response` with an error field to cancel a stream early).
        const stream = pendingStreams.get(msg.id);
        if (!stream || stream.closed) return;
        stream.closed = true;
        clearTimeout(stream.timeoutId);
        pendingStreams.delete(msg.id);
        if (msg.error) {
            const errFrame = `event: error\ndata: ${JSON.stringify({ error: msg.error, status: msg.status ?? 500 })}\n\n`;
            stream.chunks.push(new TextEncoder().encode(errFrame));
        }
        stream.resolve(concatChunks(stream.chunks));
        return;
    }
});

/**
 * Invoke the main-thread WebLLM engine's `chat_stream` and return the
 * accumulated SSE frames as a Uint8Array. Called from Rust (agent block)
 * via wasm-bindgen; see `src/blocks/agent.rs`.
 *
 * @param {string} bodyJson — OpenAI-format chat request body as a JSON string.
 * @returns {Promise<Uint8Array>} full SSE text with `event: token` frames
 *                                followed by a final `event: done` frame.
 */
export async function localLlmChatStream(bodyJson) {
    const clients = await self.clients.matchAll({
        type: 'window',
        includeUncontrolled: false,
    });
    if (clients.length === 0) {
        throw new Error('local-llm bridge: no active page — open the app to use local LLM');
    }

    let body = null;
    try { body = JSON.parse(bodyJson); } catch (_) { body = {}; }

    const id = ++requestId;

    const result = new Promise((resolve, reject) => {
        const timeoutId = setTimeout(() => {
            const stream = pendingStreams.get(id);
            if (stream && !stream.closed) {
                stream.closed = true;
                pendingStreams.delete(id);
                reject(new Error('local-llm chat_stream timed out after 5 minutes'));
            }
        }, 300_000);
        pendingStreams.set(id, {
            chunks: [],
            resolve,
            reject,
            closed: false,
            timeoutId,
        });
    });

    clients[0].postMessage({
        type: 'local-llm-request',
        id,
        action: 'chat_stream',
        body,
    });

    return result;
}

function concatChunks(chunks) {
    let total = 0;
    for (const chunk of chunks) total += chunk.length;
    const out = new Uint8Array(total);
    let off = 0;
    for (const chunk of chunks) {
        out.set(chunk, off);
        off += chunk.length;
    }
    return out;
}
