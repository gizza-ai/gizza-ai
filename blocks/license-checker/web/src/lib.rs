//! Browser-facing wasm-bindgen wrapper for /tools/license-checker/.
//! Field order MUST match page/meta.toml.
use wasm_bindgen::prelude::*;

/// Page checkboxes arrive as `"true"`/`"false"`; an empty value means the field
/// was absent, so fall back to the descriptor default.
fn truthy(s: &str, default: bool) -> bool {
    let t = s.trim().to_ascii_lowercase();
    if t.is_empty() {
        default
    } else {
        matches!(t.as_str(), "true" | "1" | "on" | "yes")
    }
}

#[allow(clippy::too_many_arguments)]
#[wasm_bindgen]
pub fn run(
    dependencies: &str,
    input_format: &str,
    allow: &str,
    deny: &str,
    exceptions: &str,
    unlisted: &str,
    unknown: &str,
    validate_ids: &str,
    include_allowed: &str,
    output: &str,
) -> Result<String, JsValue> {
    let input_format = if input_format.trim().is_empty() {
        "auto"
    } else {
        input_format
    };
    let output = if output.trim().is_empty() {
        "text"
    } else {
        output
    };
    gizza_ai_license_checker_core::check(
        dependencies,
        input_format,
        allow,
        deny,
        exceptions,
        unlisted,
        unknown,
        truthy(validate_ids, true),
        truthy(include_allowed, false),
        output,
    )
    .map_err(|e| JsValue::from_str(&e))
}
