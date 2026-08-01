//! Browser-facing wasm-bindgen wrapper for /tools/ndjson-filter/.
//! Compiled with wasm-pack for the standalone /tools/ndjson-filter/ page.
use wasm_bindgen::prelude::*;

/// Filter + reshape NDJSON.
///
/// The standalone tool page passes every field value as a string, so the
/// numeric/boolean params arrive as strings and are parsed here:
/// - `predicate` / `fields`: passed through (blank = keep all / whole records).
/// - `format`: `"ndjson"` (blank → ndjson) / `"array"` / `"csv"`.
/// - `invert` / `skip_invalid`: `"true"`/`"1"`/`"yes"`/`"on"` → on; else off.
/// - `limit`: a non-negative integer (blank/unparseable → 0 = unlimited).
///
/// Throws a JS error string on an invalid predicate, regex, format, or JSON line.
#[wasm_bindgen]
pub fn run(
    data: &str,
    predicate: &str,
    fields: &str,
    format: &str,
    invert: &str,
    limit: &str,
    skip_invalid: &str,
) -> Result<String, JsValue> {
    let invert = is_truthy(invert);
    let skip_invalid = is_truthy(skip_invalid);
    let limit = limit.trim().parse::<usize>().unwrap_or(0);
    gizza_ai_ndjson_filter_core::filter(data, predicate, fields, format, invert, limit, skip_invalid)
        .map_err(|e| JsValue::from_str(&e))
}

fn is_truthy(v: &str) -> bool {
    matches!(v.trim().to_ascii_lowercase().as_str(), "true" | "1" | "yes" | "on")
}
