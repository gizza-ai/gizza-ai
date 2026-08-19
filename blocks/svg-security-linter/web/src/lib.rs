//! Browser-facing wasm-bindgen wrapper for /tools/svg-security-linter/.
//!
//! The page driver hands every field through as a raw string, so the boolean arrives as
//! "true"/"false" and is parsed here; the core owns all other validation.
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(
    svg: &str,
    min_severity: &str,
    allow_external: &str,
    ignore: &str,
    format: &str,
) -> Result<String, JsValue> {
    let allow = matches!(
        allow_external.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "on" | "yes"
    );
    gizza_ai_svg_security_linter_core::lint(svg, min_severity, allow, ignore, format)
        .map_err(|e| JsValue::from_str(&e))
}
