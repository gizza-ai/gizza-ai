//! Browser-facing wasm-bindgen wrapper for /tools/json-path-edit/.
//! Compiled with wasm-pack for the standalone /tools/json-path-edit/ page.
use wasm_bindgen::prelude::*;

/// Get, set, or delete a value at a dotted / bracketed `path` in `json`.
///
/// The standalone tool page passes every field value as a string:
/// - `operation`: `"get"` (default) / `"set"` / `"delete"`.
/// - `value`: for `set`, the value to write (parsed as JSON, else a string).
/// - `pretty`: `"true"`/`"1"`/`"yes"`/`"on"` → indent; anything else → compact.
///
/// Throws a JS error string on invalid JSON, a bad path, or a missing value.
#[wasm_bindgen]
pub fn run(
    json: &str,
    path: &str,
    operation: &str,
    value: &str,
    pretty: &str,
) -> Result<String, JsValue> {
    let pretty = matches!(
        pretty.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "yes" | "on"
    );
    gizza_ai_json_path_edit_core::edit(json, path, operation, value, pretty)
        .map_err(|e| JsValue::from_str(&e))
}
