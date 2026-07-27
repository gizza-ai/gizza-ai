//! Browser-facing wasm-bindgen wrapper for /tools/text-to-json/.
//! Compiled with wasm-pack for the standalone /tools/text-to-json/ page.
use wasm_bindgen::prelude::*;

/// Parse structured `text` into JSON.
///
/// The standalone tool page passes every field value as a string, so the
/// boolean params arrive as strings and are parsed here:
/// - `format`: `auto` (blank) | `ini` | `keyvalue` | `logfmt` | `csv` | `passwd`.
/// - `detect_types`: `"true"`/`"1"`/`"yes"`/`"on"` → coerce booleans/numbers; else keep strings.
/// - `pretty`: `"true"`/`"1"`/`"yes"`/`"on"` → 2-space indented; else minified.
/// - `output`: `json` (blank) | `ndjson` | `report`.
///
/// Throws a JS error string on an invalid `format`/`output`, empty input, or a
/// CSV with fewer than two rows.
#[wasm_bindgen]
pub fn run(
    text: &str,
    format: &str,
    detect_types: &str,
    pretty: &str,
    output: &str,
) -> Result<String, JsValue> {
    gizza_ai_text_to_json_core::convert(
        text,
        format,
        truthy(detect_types),
        truthy(pretty),
        output,
    )
    .map_err(|e| JsValue::from_str(&e))
}

fn truthy(v: &str) -> bool {
    matches!(v.trim().to_ascii_lowercase().as_str(), "true" | "1" | "yes" | "on")
}
