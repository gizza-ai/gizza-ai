//! Browser-facing wasm-bindgen wrapper for /tools/shamir-secret-recover/.
//! Field order MUST match page/meta.toml.
use wasm_bindgen::prelude::*;

fn truthy(s: &str) -> bool {
    matches!(
        s.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "on" | "yes"
    )
}

fn parse_threshold(s: &str) -> Result<i64, JsValue> {
    let t = s.trim();
    if t.is_empty() {
        return Ok(0);
    }
    t.parse::<i64>()
        .map_err(|_| JsValue::from_str("threshold must be 0 or a whole number between 2 and 255"))
}

#[wasm_bindgen]
#[allow(clippy::too_many_arguments)]
pub fn run(
    shares: &str,
    share_format: &str,
    share_encoding: &str,
    field_poly: &str,
    threshold: &str,
    verify: &str,
    secret_encoding: &str,
    output: &str,
) -> Result<String, JsValue> {
    gizza_ai_shamir_secret_recover_core::run(
        shares,
        share_format,
        share_encoding,
        field_poly,
        parse_threshold(threshold)?,
        if verify.trim().is_empty() {
            true
        } else {
            truthy(verify)
        },
        secret_encoding,
        output,
    )
    .map_err(|e| JsValue::from_str(&e))
}
