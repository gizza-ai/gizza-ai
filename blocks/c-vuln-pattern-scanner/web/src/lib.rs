//! Browser-facing wasm-bindgen wrapper for /tools/c-vuln-pattern-scanner/.
//! Field order MUST match meta.toml: code, language, profile, min_severity,
//! ignore, format, include_context.
use wasm_bindgen::prelude::*;

/// Checkbox values arrive as strings; an empty value means "not sent" and falls
/// back to the schema default.
fn truthy(v: &str, default: bool) -> bool {
    match v.trim() {
        "" => default,
        s => matches!(s, "true" | "1" | "on" | "yes"),
    }
}

#[wasm_bindgen]
pub fn run(
    code: &str,
    language: &str,
    profile: &str,
    min_severity: &str,
    ignore: &str,
    format: &str,
    include_context: &str,
) -> Result<String, JsValue> {
    gizza_ai_c_vuln_pattern_scanner_core::scan_source(
        code,
        language,
        profile,
        min_severity,
        ignore,
        format,
        truthy(include_context, true),
    )
    .map_err(|e| JsValue::from_str(&e))
}
