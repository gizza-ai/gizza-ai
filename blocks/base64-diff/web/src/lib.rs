//! Browser-facing wasm-bindgen wrapper for /tools/base64-diff/.
//! Field ORDER must match page/meta.toml: left, right, alphabet, strict, align,
//! output, bytes_per_row, context_rows. The page hands every field over as a
//! string, so parsing lives in the core.
use gizza_ai_base64_diff_core::{diff_base64, options_from_strings};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
#[allow(clippy::too_many_arguments)]
pub fn run(
    left: &str,
    right: &str,
    alphabet: &str,
    strict: &str,
    align: &str,
    output: &str,
    bytes_per_row: &str,
    context_rows: &str,
) -> Result<String, JsValue> {
    let opts = options_from_strings(alphabet, strict, align, output, bytes_per_row, context_rows)
        .map_err(|e| JsValue::from_str(&e))?;
    diff_base64(left, right, &opts).map_err(|e| JsValue::from_str(&e))
}
