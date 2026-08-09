//! Browser-facing wasm-bindgen wrapper for /tools/base64-validator/.
//! Compiled with wasm-pack for the standalone page. Field order MUST match
//! page/meta.toml: input, variant, padding, ignore_whitespace,
//! max_line_length, output. Every field arrives as a string (checkboxes send
//! "true"/"false", number fields send their text), so they are parsed here.
use wasm_bindgen::prelude::*;

/// A checkbox that was never touched sends "" — treat that as the schema
/// default, which for `ignore_whitespace` is on.
fn truthy(s: &str, default: bool) -> bool {
    match s.trim().to_ascii_lowercase().as_str() {
        "" => default,
        "true" | "1" | "on" | "yes" => true,
        _ => false,
    }
}

/// Validate `input` and return the report (a valid/invalid verdict is a normal
/// result). Throws a JS error string only for an empty or over-size input, an
/// unknown option value, or a `max_line_length` that isn't a whole number.
#[wasm_bindgen]
pub fn run(
    input: &str,
    variant: &str,
    padding: &str,
    ignore_whitespace: &str,
    max_line_length: &str,
    output: &str,
) -> Result<String, JsValue> {
    let max_line_length = match max_line_length.trim() {
        "" => 0,
        t => t.parse::<i64>().map_err(|_| {
            JsValue::from_str(&format!(
                "invalid max_line_length {t:?}: expected a whole number, 0 (no check) to 998, \
e.g. 76 for MIME or 64 for PEM"
            ))
        })?,
    };
    gizza_ai_base64_validator_core::validate(
        input,
        variant,
        padding,
        truthy(ignore_whitespace, true),
        max_line_length,
        output,
    )
    .map_err(|e| JsValue::from_str(&e))
}
