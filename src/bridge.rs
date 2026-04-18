//! gizza-ai ↔ browser JS bridge via wasm-bindgen.
//!
//! Exposes SW-internal endpoints to Rust wasm blocks. Today this only
//! covers local-llm's chat_stream, which is implemented in sw.js (not
//! in the Rust runtime) and therefore can't be reached via
//! ctx.call_block.
//!
//! The JS side calls globalThis.__gizzaHandleLocalLlm (a reference to
//! handleLocalLlm in sw.js, set at module init time) with a synthetic
//! Request and collects the full SSE response into a Uint8Array.

use wasm_bindgen::prelude::*;

#[wasm_bindgen(module = "/site/bridge.js")]
extern "C" {
    /// Invoke the SW's chat_stream handler and collect the SSE response
    /// into a Uint8Array.
    ///
    /// `body_json` must be a JSON-encoded string like
    /// `{"messages":[...],"tools":[...]}`.
    ///
    /// On success, the returned `JsValue` is a `Uint8Array` containing
    /// the full SSE text. Convert with:
    /// ```ignore
    /// let bytes = js_sys::Uint8Array::new(&js_result).to_vec();
    /// ```
    ///
    /// On failure the Promise is rejected and the error is returned as
    /// the `Err(JsValue)` variant.
    #[wasm_bindgen(js_name = localLlmChatStream, catch)]
    pub async fn local_llm_chat_stream(body_json: &str) -> Result<JsValue, JsValue>;
}
