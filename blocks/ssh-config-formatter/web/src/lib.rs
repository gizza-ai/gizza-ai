//! Browser-facing wasm-bindgen wrapper for /tools/ssh-config-formatter/.
//! Field order MUST match meta.toml: text, output, indent, keyword_case,
//! align_values, sort_keywords, dedupe, include_notes, min_severity.
//! Fields arrive as strings (checkboxes send "true"/"false").
use wasm_bindgen::prelude::*;

fn truthy(s: &str) -> bool {
    matches!(
        s.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "on" | "yes"
    )
}

/// A blank checkbox field never reaches the page driver, but a deep link may omit
/// it — fall back to the schema default rather than silently flipping it off.
fn checkbox(s: &str, default: bool) -> bool {
    if s.trim().is_empty() {
        default
    } else {
        truthy(s)
    }
}

fn parse_indent(s: &str) -> Result<i64, JsValue> {
    let t = s.trim();
    if t.is_empty() {
        return Ok(2);
    }
    t.parse::<i64>()
        .map_err(|_| JsValue::from_str("indent must be a whole number between 0 and 8"))
}

#[wasm_bindgen]
#[allow(clippy::too_many_arguments)]
pub fn run(
    text: &str,
    output: &str,
    indent: &str,
    keyword_case: &str,
    align_values: &str,
    sort_keywords: &str,
    dedupe: &str,
    include_notes: &str,
    min_severity: &str,
) -> Result<String, JsValue> {
    gizza_ai_ssh_config_formatter_core::run(
        text,
        output,
        parse_indent(indent)?,
        keyword_case,
        checkbox(align_values, false),
        checkbox(sort_keywords, false),
        checkbox(dedupe, false),
        checkbox(include_notes, true),
        min_severity,
    )
    .map_err(|e| JsValue::from_str(&e))
}
