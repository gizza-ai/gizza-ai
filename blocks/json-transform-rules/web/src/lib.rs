//! Browser-facing wasm-bindgen wrapper for /tools/json-transform-rules/.
//! Compiled with wasm-pack for the standalone /tools/json-transform-rules/ page.
use wasm_bindgen::prelude::*;

/// Reshape a JSON document with declarative mapping rules.
///
/// The standalone tool page passes every field value as a string, so the
/// numeric/boolean params arrive as strings and are parsed here:
/// - `json` / `rules`: passed through (both required by the core).
/// - `each`: blank runs the rules once against the whole document.
/// - `mode`: `"build"` (blank → build) / `"merge"`.
/// - `on_missing`: `"skip"` (blank → skip) / `"null"` / `"error"`.
/// - `array_mode`: `"auto"` (blank → auto) / `"always"` / `"first"`.
/// - `pretty`: `"true"`/`"1"`/`"yes"`/`"on"` → on; blank/anything else → off
///   (the page checkbox is checked by default, so blank only reaches here when
///   the user unticks it).
/// - `indent`: 0–8 spaces (blank/unparseable → 2).
/// - `output`: `"json"` (blank → json) / `"report"`.
///
/// Throws a JS error string on invalid JSON input, an unparseable rule, a bad
/// selector or target path, an unknown enum value, or a transform applied to a
/// value it cannot handle.
#[allow(clippy::too_many_arguments)]
#[wasm_bindgen]
pub fn run(
    json: &str,
    rules: &str,
    each: &str,
    mode: &str,
    on_missing: &str,
    array_mode: &str,
    pretty: &str,
    indent: &str,
    output: &str,
) -> Result<String, JsValue> {
    gizza_ai_json_transform_rules_core::transform(
        json,
        rules,
        each,
        mode,
        on_missing,
        array_mode,
        is_truthy(pretty),
        parse_usize(indent, 2),
        output,
    )
    .map_err(|e| JsValue::from_str(&e))
}

fn parse_usize(v: &str, fallback: usize) -> usize {
    match v.trim() {
        "" => fallback,
        s => s.parse::<usize>().unwrap_or(fallback),
    }
}

fn is_truthy(v: &str) -> bool {
    matches!(
        v.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "yes" | "on"
    )
}
