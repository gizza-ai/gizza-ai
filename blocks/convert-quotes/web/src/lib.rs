//! Browser-facing wasm-bindgen wrapper for /tools/convert-quotes/.
//!
//! The page driver hands every field through as a raw string, so each param is
//! taken as `&str` and parsed here; the core owns all validation so the page,
//! the CLI and chat funnel through exactly the same rules.
use wasm_bindgen::prelude::*;

/// Page checkboxes arrive as "true"/"false"; treat any positive spelling as on.
fn truthy(v: &str, default: bool) -> bool {
    match v.trim() {
        "" => default,
        s => matches!(s, "true" | "1" | "on" | "yes"),
    }
}

#[wasm_bindgen]
pub fn run(
    input: &str,
    direction: &str,
    escape_style: &str,
    preserve_apostrophes: &str,
    on_unbalanced: &str,
    include_report: &str,
) -> Result<String, JsValue> {
    gizza_ai_convert_quotes_core::run(
        input,
        direction,
        escape_style,
        truthy(preserve_apostrophes, true),
        on_unbalanced,
        truthy(include_report, false),
    )
    .map_err(|e| JsValue::from_str(&e))
}
