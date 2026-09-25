//! Browser-facing wasm-bindgen wrapper for /tools/python-syntax-check/.
//! Field order MUST match page/meta.toml: code, mode, format, show_context,
//! python2_hints, stats, filename. Every field arrives as a string (checkboxes
//! marshal their checked state as "true"/"false").
use gizza_ai_python_syntax_check_core::run_with_options;
use wasm_bindgen::prelude::*;

/// A checkbox field, falling back to the schema default when the field is absent
/// or blank (a deep link may omit it).
fn flag(s: &str, default: bool) -> bool {
    match s.trim().to_ascii_lowercase().as_str() {
        "" => default,
        "true" | "1" | "on" | "yes" => true,
        _ => false,
    }
}

#[wasm_bindgen]
pub fn run(
    code: &str,
    mode: &str,
    format: &str,
    show_context: &str,
    python2_hints: &str,
    stats: &str,
    filename: &str,
) -> Result<String, JsValue> {
    run_with_options(
        code,
        mode,
        format,
        flag(show_context, true),
        flag(python2_hints, true),
        flag(stats, true),
        filename,
    )
    .map_err(|e| JsValue::from_str(&e))
}
