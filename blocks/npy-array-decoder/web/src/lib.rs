//! Browser-facing wasm-bindgen wrapper for /tools/npy-array-decoder/.
//! Compiled with wasm-pack for the standalone /tools/npy-array-decoder/ page.
//!
//! Field order MUST match meta.toml: input, input_format, output, limit,
//! delimiter. The page passes every field value as a string.
use wasm_bindgen::prelude::*;

/// Decode a NumPy `.npy` file and render its dtype, shape and values.
///
/// - `input`: the file bytes as base64 or hex (a `data:` URI prefix is ignored).
/// - `input_format`: `"auto"` (default), `"base64"` or `"hex"` (blank → auto).
/// - `output`: `"summary"` (default), `"json"`, `"csv"` or `"header"`.
/// - `limit`: max values rendered, 1..=100000 (blank/0 → 1000).
/// - `delimiter`: CSV separator — a single character or `"tab"` (blank → `,`).
///
/// Throws a JS error string on invalid arguments or an undecodable file.
#[wasm_bindgen]
pub fn run(
    input: &str,
    input_format: &str,
    output: &str,
    limit: &str,
    delimiter: &str,
) -> Result<String, JsValue> {
    let n: usize = limit.trim().parse().unwrap_or(0);
    let delimiter = if delimiter.is_empty() { "," } else { delimiter };
    gizza_ai_npy_array_decoder_core::run(input, input_format, output, n, delimiter)
        .map_err(|e| JsValue::from_str(&e))
}
