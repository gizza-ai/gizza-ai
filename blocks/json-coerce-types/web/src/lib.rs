//! Browser-facing wasm-bindgen wrapper for /tools/json-coerce-types/.
//! Argument order MUST match the descriptor param order and page/meta.toml:
//! input, numbers, booleans, nulls, bool_synonyms, null_tokens, empty_strings,
//! trim, leading_zeros, thousands, skip_keys, only_keys, indent, output.
//! tool.js hands every field over as a raw string (checkboxes as
//! "true"/"false"), so each one is parsed here.
use wasm_bindgen::prelude::*;

/// A checkbox that defaults to CHECKED: a blank value (param absent from the
/// query string) still means "on".
fn on_by_default(s: &str) -> bool {
    !matches!(
        s.trim().to_ascii_lowercase().as_str(),
        "false" | "0" | "off" | "no"
    )
}

/// A checkbox that defaults to UNCHECKED: only an explicit positive turns it on.
fn off_by_default(s: &str) -> bool {
    matches!(
        s.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "on" | "yes"
    )
}

#[wasm_bindgen]
#[allow(clippy::too_many_arguments)]
pub fn run(
    input: &str,
    numbers: &str,
    booleans: &str,
    nulls: &str,
    bool_synonyms: &str,
    null_tokens: &str,
    empty_strings: &str,
    trim: &str,
    leading_zeros: &str,
    thousands: &str,
    skip_keys: &str,
    only_keys: &str,
    indent: &str,
    output: &str,
) -> Result<String, JsValue> {
    // A blank indent field means "use the schema default", not "minify".
    let indent: usize = match indent.trim() {
        "" => 2,
        s => s.parse().unwrap_or(2),
    };
    gizza_ai_json_coerce_types_core::run(
        input,
        on_by_default(numbers),
        on_by_default(booleans),
        on_by_default(nulls),
        off_by_default(bool_synonyms),
        null_tokens,
        empty_strings,
        off_by_default(trim),
        leading_zeros,
        off_by_default(thousands),
        skip_keys,
        only_keys,
        indent,
        output,
    )
    .map_err(|e| JsValue::from_str(&e))
}
