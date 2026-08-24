//! Browser-facing wasm-bindgen wrapper for /tools/xliff-to-json/.
//! Field order MUST match meta.toml: xliff, output, key, inline_tags,
//! include_empty_targets, fallback_to_source, nested, separator,
//! include_metadata.
use wasm_bindgen::prelude::*;

fn truthy(v: &str) -> bool {
    matches!(v.trim().to_ascii_lowercase().as_str(), "true" | "1" | "on" | "yes")
}

fn flag(v: &str, default: bool) -> bool {
    if v.trim().is_empty() {
        default
    } else {
        truthy(v)
    }
}

/// Blank enum/text fields fall back to the descriptor default so a page load
/// with a partially-filled query string still runs.
fn or_default<'a>(v: &'a str, fallback: &'a str) -> &'a str {
    if v.trim().is_empty() {
        fallback
    } else {
        v.trim()
    }
}

#[wasm_bindgen]
#[allow(clippy::too_many_arguments)]
pub fn run(
    xliff: &str,
    output: &str,
    key: &str,
    inline_tags: &str,
    include_empty_targets: &str,
    fallback_to_source: &str,
    nested: &str,
    separator: &str,
    include_metadata: &str,
) -> Result<String, JsValue> {
    gizza_ai_xliff_to_json_core::run(
        xliff,
        or_default(output, "pairs"),
        or_default(key, "id"),
        truthy(nested),
        if separator.is_empty() { "." } else { separator },
        flag(include_empty_targets, true),
        flag(fallback_to_source, false),
        or_default(inline_tags, "placeholder"),
        flag(include_metadata, false),
    )
    .map_err(|e| JsValue::from_str(&e))
}
