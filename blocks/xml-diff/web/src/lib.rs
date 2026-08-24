//! Browser-facing wasm-bindgen wrapper for /tools/xml-diff/.
//! Field order MUST match meta.toml: left, right, strategy, ignore_whitespace,
//! ignore_comments, ignore_namespaces, numeric_text, format, indent.
use gizza_ai_xml_diff_core::diff_raw;
use wasm_bindgen::prelude::*;

/// Page checkboxes arrive as "true"/"false" — parse positive-truthy.
fn flag(v: &str, default: bool) -> bool {
    match v.trim().to_ascii_lowercase().as_str() {
        "true" | "1" | "on" | "yes" => true,
        "false" | "0" | "off" | "no" => false,
        _ => default,
    }
}

#[wasm_bindgen]
#[allow(clippy::too_many_arguments)]
pub fn run(
    left: &str,
    right: &str,
    strategy: &str,
    ignore_whitespace: &str,
    ignore_comments: &str,
    ignore_namespaces: &str,
    numeric_text: &str,
    format: &str,
    indent: &str,
) -> Result<String, JsValue> {
    let n: usize = indent.trim().parse().unwrap_or(2);
    diff_raw(
        left,
        right,
        strategy,
        flag(ignore_whitespace, true),
        flag(ignore_comments, true),
        flag(ignore_namespaces, false),
        flag(numeric_text, false),
        format,
        n,
    )
    .map_err(|e| JsValue::from_str(&e))
}
