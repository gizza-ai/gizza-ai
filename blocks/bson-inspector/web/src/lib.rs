//! Browser-facing wasm-bindgen wrapper for /tools/bson-inspector/.
//! Compiled with wasm-pack for the standalone /tools/bson-inspector/ page.
//!
//! Field order MUST match meta.toml: input, input_format, output, indent,
//! show_offsets. The page passes every field value as a string (checkboxes
//! arrive as "true"/"false").
use wasm_bindgen::prelude::*;

fn truthy(s: &str) -> bool {
    matches!(s.trim().to_ascii_lowercase().as_str(), "true" | "1" | "on" | "yes")
}

/// Parse a BSON document into a typed tree or canonical Extended JSON v2.
///
/// - `input`: the BSON bytes as a base64 or hex string.
/// - `input_format`: `"base64"` (default) or `"hex"` (blank → base64).
/// - `output`: `"tree"` (default) or `"json"` (blank → tree).
/// - `indent`: spaces per nesting level, 0-8 (blank → 2).
/// - `show_offsets`: `"true"`/`"false"` — prefix tree lines with byte offsets.
///
/// Throws a JS error string on invalid arguments or undecodable input.
#[wasm_bindgen]
pub fn run(
    input: &str,
    input_format: &str,
    output: &str,
    indent: &str,
    show_offsets: &str,
) -> Result<String, JsValue> {
    let n: usize = indent.trim().parse().unwrap_or(2);
    gizza_ai_bson_inspector_core::run(input, input_format, output, n, truthy(show_offsets))
        .map_err(|e| JsValue::from_str(&e))
}
