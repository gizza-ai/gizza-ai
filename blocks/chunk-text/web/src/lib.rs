//! Browser-facing wasm-bindgen wrapper for /tools/chunk-text/.
//! Compiled with wasm-pack for the standalone page. The page passes every field
//! value as a string; blank numeric fields fall back to the descriptor defaults
//! and the core validates ranges.
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(
    text: &str,
    chunk_size: &str,
    overlap: &str,
    unit: &str,
    boundary: &str,
    chars_per_token: &str,
    format: &str,
    trim: &str,
) -> Result<String, JsValue> {
    let chunk_size = chunk_size.trim().parse::<usize>().unwrap_or(500);
    let overlap = overlap.trim().parse::<usize>().unwrap_or(50);
    let chars_per_token = chars_per_token.trim().parse::<f64>().unwrap_or(4.0);
    let truthy =
        |v: &str| matches!(v.trim().to_ascii_lowercase().as_str(), "true" | "1" | "on" | "yes");
    gizza_ai_chunk_text_core::chunk(
        text,
        chunk_size,
        overlap,
        unit,
        boundary,
        chars_per_token,
        format,
        truthy(trim),
    )
    .map_err(|e| JsValue::from_str(&e))
}
