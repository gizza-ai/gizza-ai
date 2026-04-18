// bridge.js — gizza-ai JS functions exposed to Rust via wasm-bindgen.
//
// These functions bridge SW-internal endpoints to Rust wasm blocks.
// Today this only covers local-llm's chat_stream, which is implemented
// in sw.js (not the Rust runtime) and can't be reached via ctx.call_block.

/**
 * Invoke the SW's chat_stream handler directly, collecting the SSE response
 * into a single byte array that the Rust agent block can parse.
 *
 * Returns a Uint8Array containing the full SSE text, or throws on error.
 *
 * @param {string} bodyJson - JSON-encoded string like {"messages":[...],"tools":[...]}
 * @returns {Promise<Uint8Array>}
 */
export async function localLlmChatStream(bodyJson) {
    if (typeof globalThis.__gizzaHandleLocalLlm !== 'function') {
        throw new Error('__gizzaHandleLocalLlm not available — sw.js must set it');
    }
    const request = new Request('/b/local-llm/api/chat_stream', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: bodyJson,
    });
    const response = await globalThis.__gizzaHandleLocalLlm(request);
    if (!response.ok) {
        const text = await response.text();
        throw new Error(`chat_stream HTTP ${response.status}: ${text}`);
    }
    const buf = await response.arrayBuffer();
    return new Uint8Array(buf);
}
