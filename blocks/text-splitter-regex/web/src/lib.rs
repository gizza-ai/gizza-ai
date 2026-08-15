//! Browser-facing wasm-bindgen wrapper for /tools/text-splitter-regex/.
//! Field order MUST match page/meta.toml: text, pattern, field_pattern, output,
//! separator, max_splits, ignore_case, multiline, dotall, trim, remove_empty.
//! The page passes every field value as a string, so numbers and booleans are
//! parsed here.
use gizza_ai_text_splitter_regex_core::split;
use wasm_bindgen::prelude::*;

fn truthy(v: &str) -> bool {
    matches!(
        v.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "on" | "yes"
    )
}

/// Split `text` on the regular expression `pattern`.
///
/// Throws a JS error string on an invalid pattern/output format, on empty text
/// or an empty pattern, or when the input exceeds the documented limits.
#[wasm_bindgen]
#[allow(clippy::too_many_arguments)]
pub fn run(
    text: &str,
    pattern: &str,
    field_pattern: &str,
    output: &str,
    separator: &str,
    max_splits: &str,
    ignore_case: &str,
    multiline: &str,
    dotall: &str,
    trim: &str,
    remove_empty: &str,
) -> Result<String, JsValue> {
    // Blank / non-numeric max_splits means "unlimited", matching the schema
    // default; the page renders it as a number box that starts empty.
    let max_splits: usize = max_splits.trim().parse().unwrap_or(0);
    // The page does not pre-fill schema defaults into text boxes, so a blank
    // separator falls back to the documented default rather than joining the
    // parts with nothing.
    let separator = if separator.is_empty() { ", " } else { separator };
    split(
        text,
        pattern,
        field_pattern,
        truthy(ignore_case),
        truthy(multiline),
        truthy(dotall),
        truthy(trim),
        truthy(remove_empty),
        max_splits,
        output,
        separator,
    )
    .map_err(|e| JsValue::from_str(&e))
}
