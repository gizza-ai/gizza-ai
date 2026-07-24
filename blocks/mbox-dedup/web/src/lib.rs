//! Browser-facing wasm-bindgen wrapper for /tools/mbox-dedup/.
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(mbox: &str, keep: &str, ignore_case: &str, no_message_id: &str) -> Result<String, JsValue> {
    let keep = if keep.is_empty() { "first" } else { keep };
    let ignore_case = matches!(ignore_case, "true" | "1" | "on" | "yes");
    let no_message_id = if no_message_id.is_empty() { "keep" } else { no_message_id };
    gizza_ai_mbox_dedup_core::run(mbox, keep, ignore_case, no_message_id)
        .map_err(|e| JsValue::from_str(&e))
}
