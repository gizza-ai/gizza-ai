//! Browser-facing wasm-bindgen wrapper for /tools/mbox-splitter/.
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(
    mbox: &str,
    output: &str,
    naming: &str,
    message: &str,
    unescape_from: &str,
    keep_postmark: &str,
) -> Result<String, JsValue> {
    let output = if output.is_empty() { "files" } else { output };
    let naming = if naming.is_empty() { "index" } else { naming };
    let message: i64 = if message.trim().is_empty() {
        0
    } else {
        message
            .trim()
            .parse()
            .map_err(|_| JsValue::from_str("message must be a whole number (0 = every message)"))?
    };
    let unescape_from = matches!(unescape_from, "true" | "1" | "on" | "yes");
    let keep_postmark = matches!(keep_postmark, "true" | "1" | "on" | "yes");
    gizza_ai_mbox_splitter_core::run(mbox, output, naming, message, unescape_from, keep_postmark)
        .map_err(|e| JsValue::from_str(&e))
}
