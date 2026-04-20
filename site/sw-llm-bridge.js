// sw-llm-bridge.js — SW-context wrapper for gizza's local-LLM chat_stream.
//
// This module is loaded by the wasm-bindgen glue at Service-Worker import
// time (via agent.rs's `#[wasm_bindgen(module = "/site/sw-llm-bridge.js")]`).
// It must NOT reference WebLLM, WebGPU, or any CDN — those all live in
// main-thread-only ai-bridge.js. Keeping them separate is what lets the
// SW module graph parse without failing on a cross-origin ES import.
//
// The function forwards the request through the framework's SW local-LLM
// bridge: the SW installs `globalThis.__gizzaHandleLocalLlm` during init,
// and that handler postMessages the request to a main-thread client where
// ai-bridge.js runs the actual inference on WebGPU.

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
