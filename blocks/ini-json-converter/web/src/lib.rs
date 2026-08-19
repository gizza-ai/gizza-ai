//! Browser-facing wasm-bindgen wrapper for /tools/ini-json-converter/.
//! Compiled with wasm-pack for the standalone /tools/ini-json-converter/ page.
use wasm_bindgen::prelude::*;

/// Convert INI ⇄ JSON, or get/set/delete one key in INI text.
///
/// The standalone tool page passes every field value as a string, so the boolean
/// params arrive as strings and are parsed here. Argument order must match the
/// `[[input]]` order in `page/meta.toml` — the page runtime spreads the fields
/// into this call positionally:
/// - `mode`: `auto` (blank) | `ini_to_json` | `json_to_ini` | `get` | `set` | `delete`.
/// - `section` / `key` / `value`: the target and new value of a get/set/delete.
/// - `detect_types`: `"true"`/`"1"`/`"yes"`/`"on"` → coerce booleans/numbers; else off.
/// - `inline_comments`: same truthiness; strips a trailing ` ; note` from a value.
/// - `delimiter`: `equals_spaced` (blank) | `equals` | `colon`.
/// - `pretty`: same truthiness; indents JSON output.
/// - `indent`: `2` (blank) | `4` | `tab`.
///
/// Throws a JS error string on an invalid `mode`/`delimiter`/`indent`, a malformed
/// INI line, invalid JSON, a missing key/section, or a value with no INI form.
#[wasm_bindgen]
#[allow(clippy::too_many_arguments)]
pub fn run(
    input: &str,
    mode: &str,
    section: &str,
    key: &str,
    value: &str,
    detect_types: &str,
    inline_comments: &str,
    delimiter: &str,
    pretty: &str,
    indent: &str,
) -> Result<String, JsValue> {
    gizza_ai_ini_json_converter_core::convert(
        input,
        mode,
        section,
        key,
        value,
        truthy(detect_types),
        truthy(inline_comments),
        delimiter,
        truthy(pretty),
        indent,
    )
    .map_err(|e| JsValue::from_str(&e))
}

fn truthy(v: &str) -> bool {
    matches!(v.trim().to_ascii_lowercase().as_str(), "true" | "1" | "yes" | "on")
}
