//! Browser-facing wasm-bindgen wrapper for /tools/midi-to-json/.
//! Field order MUST match meta.toml: input, encoding, format. Empty select
//! fields fall back to the schema defaults.
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(input: &str, encoding: &str, format: &str) -> Result<String, JsValue> {
    let enc = if encoding.trim().is_empty() { "auto" } else { encoding };
    let fmt = if format.trim().is_empty() { "notes" } else { format };
    gizza_ai_midi_to_json_core::convert(input, enc, fmt).map_err(|e| JsValue::from_str(&e))
}
