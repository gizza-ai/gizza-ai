//! Browser-facing wasm-bindgen wrapper for /tools/charset-decoder/.
//!
//! Field order MUST match page/meta.toml: input, input_format, charset, output,
//! errors, strip_bom, per_line. Each value arrives as a string from the generic
//! page runtime; booleans use positive-truthy parsing to match checkbox and
//! query-param forms.
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(
    input: &str,
    input_format: &str,
    charset: &str,
    output: &str,
    errors: &str,
    strip_bom: &str,
    per_line: &str,
) -> Result<String, JsValue> {
    let decoded = gizza_ai_charset_decoder_core::run(
        input,
        input_format,
        charset,
        output,
        errors,
        truthy(strip_bom, true),
        truthy(per_line, false),
    )
    .map_err(|e| JsValue::from_str(&e))?;
    Ok(decoded.text)
}

fn truthy(s: &str, default: bool) -> bool {
    let t = s.trim().to_ascii_lowercase();
    if t.is_empty() {
        default
    } else {
        matches!(t.as_str(), "true" | "1" | "on" | "yes")
    }
}
