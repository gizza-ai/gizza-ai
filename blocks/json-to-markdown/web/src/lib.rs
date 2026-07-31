//! Browser-facing wasm-bindgen wrapper for /tools/json-to-markdown/.
//! Compiled with wasm-pack for the standalone /tools/json-to-markdown/ page.
use wasm_bindgen::prelude::*;

/// Render `json` as nested Markdown.
///
/// The standalone tool page passes every field value as a string, so the
/// numeric/boolean params arrive as strings and are parsed here:
/// - `heading_level`: `1`–`6` (blank/unparseable → 2; the core clamps the range).
/// - `array_mode`: `"auto"`/`"table"`/`"list"` (blank → auto).
/// - `sort_keys`: `"true"`/`"1"`/`"yes"`/`"on"` → sort; anything else → off.
/// - `max_depth`: `1`–`20` (blank/unparseable → 6; the core clamps the range).
///
/// Throws a JS error string on invalid JSON or an invalid `array_mode`.
#[wasm_bindgen]
pub fn run(
    json: &str,
    heading_level: &str,
    array_mode: &str,
    sort_keys: &str,
    max_depth: &str,
) -> Result<String, JsValue> {
    let heading_level = heading_level.trim().parse::<u32>().unwrap_or(2);
    let sort_keys = matches!(
        sort_keys.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "yes" | "on"
    );
    let max_depth = max_depth.trim().parse::<u32>().unwrap_or(6);
    gizza_ai_json_to_markdown_core::to_markdown(
        json,
        heading_level,
        array_mode,
        sort_keys,
        max_depth,
    )
    .map_err(|e| JsValue::from_str(&e))
}
