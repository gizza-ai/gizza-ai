//! Browser-facing wasm-bindgen wrapper for /tools/http-headers-diff/.
//! The standalone page passes every field value as a string; the boolean
//! checkbox arrives as "true"/"false" and is parsed here. Field order MUST
//! match page/meta.toml: left, right, ignore, ignore_order, output.
use wasm_bindgen::prelude::*;

/// Diff two sets of HTTP headers.
///
/// - `left`: the first (old/base) header block (required).
/// - `right`: the second (new/compared) header block (required).
/// - `ignore`: header names to exclude (comma/space/newline separated; blank → none).
/// - `ignore_order`: `"true"`/`"1"`/`"yes"`/`"on"` compares comma-list values as a
///   set (default-false checkbox — anything else is an exact string compare).
/// - `output`: `report` (default) | `json` (blank → report).
///
/// Throws a JS error string on an unparseable block or an invalid `output` mode.
#[wasm_bindgen]
pub fn run(
    left: &str,
    right: &str,
    ignore: &str,
    ignore_order: &str,
    output: &str,
) -> Result<String, JsValue> {
    let ignore_order = matches!(
        ignore_order.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "yes" | "on"
    );
    gizza_ai_http_headers_diff_core::diff(left, right, ignore, ignore_order, output)
        .map_err(|e| JsValue::from_str(&e))
}
