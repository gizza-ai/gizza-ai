//! Browser-facing wasm-bindgen wrapper for /tools/code-outline-extractor/.
use wasm_bindgen::prelude::*;

/// Parse a checkbox string ("", "true"/"1"/"on"/"yes") into a bool, defaulting
/// to `default` when the field is blank.
fn parse_bool(s: &str, default: bool) -> bool {
    match s.trim().to_ascii_lowercase().as_str() {
        "" => default,
        "true" | "1" | "on" | "yes" => true,
        _ => false,
    }
}

#[wasm_bindgen]
pub fn run(
    code: &str,
    language: &str,
    format: &str,
    signatures: &str,
    line_numbers: &str,
) -> Result<String, JsValue> {
    let signatures = parse_bool(signatures, false);
    let line_numbers = parse_bool(line_numbers, true);
    gizza_ai_code_outline_extractor_core::outline(code, language, format, signatures, line_numbers)
        .map_err(|e| JsValue::from_str(&e))
}
