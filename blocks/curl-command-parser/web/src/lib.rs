//! Browser-facing wasm-bindgen wrapper for /tools/curl-command-parser/.
//! Field order MUST match meta.toml: mode, command. `human = true` → parse mode
//! renders the aligned human-readable view (rebuild mode returns the command
//! either way).
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(mode: &str, command: &str) -> Result<String, JsValue> {
    gizza_ai_curl_command_parser_core::dispatch(mode, command, true)
        .map_err(|e| JsValue::from_str(&e))
}
