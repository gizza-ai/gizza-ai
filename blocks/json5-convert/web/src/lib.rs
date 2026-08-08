//! Browser-facing wasm-bindgen wrapper for /tools/json5-convert/.
//! Compiled with wasm-pack for the standalone /tools/json5-convert/ page.
use wasm_bindgen::prelude::*;

/// Convert `text` between JSON5/JSONC and strict JSON.
///
/// The standalone tool page passes every field value as a string, so the
/// boolean params arrive as `"true"`/`"false"` and are parsed here; blank
/// enum values fall through to the core's documented defaults:
/// - `direction`: `"to-json"` (blank) / `"to-json5"` / `"auto"`.
/// - `indent`: `"2"` (blank) / `"4"` / `"tab"` / `"minify"`.
/// - `sort_keys`, `unquote_keys`, `trailing_commas`: `"true"`/`"1"`/`"yes"`/`"on"`.
///   `unquote_keys` is the one checkbox that defaults ON, so a blank value
///   (field absent) keeps it on while an explicit `"false"` turns it off.
/// - `nonfinite`: `"null"` (blank) / `"string"` / `"error"`.
/// - `quote_style`: `"single"` (blank) / `"double"`.
///
/// Throws a JS error string on invalid syntax (with the line and column), an
/// unknown option value, empty input, or input over the 1 MB / 200-level caps.
#[wasm_bindgen]
pub fn run(
    text: &str,
    direction: &str,
    indent: &str,
    sort_keys: &str,
    nonfinite: &str,
    quote_style: &str,
    unquote_keys: &str,
    trailing_commas: &str,
) -> Result<String, JsValue> {
    gizza_ai_json5_convert_core::convert(
        text,
        direction,
        indent,
        is_on(sort_keys, false),
        nonfinite,
        quote_style,
        is_on(unquote_keys, true),
        is_on(trailing_commas, false),
    )
    .map_err(|e| JsValue::from_str(&e))
}

/// Positive-truthy checkbox parsing; a blank value means "not supplied" and
/// falls back to the param's descriptor default.
fn is_on(value: &str, default: bool) -> bool {
    let v = value.trim();
    if v.is_empty() {
        return default;
    }
    matches!(v.to_ascii_lowercase().as_str(), "true" | "1" | "yes" | "on")
}
