//! Browser-facing wasm-bindgen wrapper for /tools/ini-parser/.
//! Compiled with wasm-pack for the standalone /tools/ini-parser/ page.
use wasm_bindgen::prelude::*;

/// Parse INI/conf `ini` text into structured JSON.
///
/// The standalone tool page passes every field value as a string, so the
/// boolean params arrive as strings and are parsed here:
/// - `output`: `json` (blank) | `flat` | `report`.
/// - `duplicate_keys`: `last` (blank) | `first` | `array` | `error`.
/// - `detect_types`: `"true"`/`"1"`/`"yes"`/`"on"` → coerce booleans/numbers; else off.
/// - `comments`: `both` (blank) | `semicolon` | `hash`.
/// - `inline_comments`: `"true"`/`"1"`/`"yes"`/`"on"` → strip trailing comments; else off.
///
/// Throws a JS error string on an invalid `output`/`duplicate_keys`/`comments`,
/// a malformed line, or (with `duplicate_keys=error`) a duplicate key.
#[wasm_bindgen]
pub fn run(
    ini: &str,
    output: &str,
    duplicate_keys: &str,
    detect_types: &str,
    comments: &str,
    inline_comments: &str,
) -> Result<String, JsValue> {
    gizza_ai_ini_parser_core::parse(
        ini,
        output,
        duplicate_keys,
        truthy(detect_types),
        comments,
        truthy(inline_comments),
    )
    .map_err(|e| JsValue::from_str(&e))
}

fn truthy(v: &str) -> bool {
    matches!(v.trim().to_ascii_lowercase().as_str(), "true" | "1" | "yes" | "on")
}
