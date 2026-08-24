//! Browser-facing wasm-bindgen wrapper for /tools/json-trim-strings/.
//! Field order MUST match page/meta.toml: input, trim, internal, whitespace,
//! keys, empty, only_keys, skip_keys, indent. Every field arrives as a string
//! (checkboxes send "true"/"false", numbers arrive as text); the core owns all
//! validation and error messages.
use wasm_bindgen::prelude::*;

fn truthy(s: &str) -> bool {
    matches!(
        s.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "on" | "yes"
    )
}

/// Blank / unparseable falls back to the descriptor default; the core clamps.
fn indent_of(s: &str) -> i64 {
    match s.trim() {
        "" => 2,
        t => t.parse::<i64>().unwrap_or(2),
    }
}

/// Trim/normalize the whitespace of every string value in a JSON document.
///
/// - `input`: the JSON document text.
/// - `trim`: `both` | `leading` | `trailing` | `none`.
/// - `internal`: `keep` | `collapse` | `remove`.
/// - `whitespace`: `unicode` (includes NBSP and friends) | `ascii`.
/// - `keys`: checkbox `"true"`/`"false"` — also rewrite object keys.
/// - `empty`: `keep` | `null` | `drop` for a value the trim empties.
/// - `only_keys` / `skip_keys`: comma-separated key names (empty → all / none).
/// - `indent`: spaces per level, `0` minifies.
///
/// Throws a JS error string on invalid JSON or an unknown option.
#[allow(clippy::too_many_arguments)]
#[wasm_bindgen]
pub fn run(
    input: &str,
    trim: &str,
    internal: &str,
    whitespace: &str,
    keys: &str,
    empty: &str,
    only_keys: &str,
    skip_keys: &str,
    indent: &str,
) -> Result<String, JsValue> {
    gizza_ai_json_trim_strings_core::trim_strings(
        input,
        trim,
        internal,
        whitespace,
        truthy(keys),
        empty,
        only_keys,
        skip_keys,
        indent_of(indent),
    )
    .map_err(|e| JsValue::from_str(&e))
}
