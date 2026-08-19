//! Browser-facing wasm-bindgen wrapper for /tools/ndjson-viewer/.
//! Field order MUST match meta.toml: data, view, indent, search, search_in,
//! case_sensitive, path, value, match_mode, sort_keys, skip, limit, invalid,
//! line_numbers, stats. Every field arrives as a string (checkboxes send
//! "true"/"false", and an empty box means "leave it at the default").
use wasm_bindgen::prelude::*;

fn truthy(s: &str) -> bool {
    matches!(
        s.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "on" | "yes"
    )
}

/// An empty field means "the default" — the page clears a box rather than
/// deleting the param, so the core must not see "".
fn or_default<'a>(value: &'a str, fallback: &'a str) -> &'a str {
    if value.trim().is_empty() {
        fallback
    } else {
        value
    }
}

/// A whole-number field: empty means the default, anything unparseable is a
/// message naming the field rather than a silent fallback.
fn whole(value: &str, fallback: i64, what: &str) -> Result<i64, JsValue> {
    let text = value.trim();
    if text.is_empty() {
        return Ok(fallback);
    }
    text.parse::<i64>()
        .map_err(|_| JsValue::from_str(&format!("{what}, got \"{text}\"")))
}

#[wasm_bindgen]
#[allow(clippy::too_many_arguments)]
pub fn run(
    data: &str,
    view: &str,
    indent: &str,
    search: &str,
    search_in: &str,
    case_sensitive: &str,
    path: &str,
    value: &str,
    match_mode: &str,
    sort_keys: &str,
    skip: &str,
    limit: &str,
    invalid: &str,
    line_numbers: &str,
    stats: &str,
) -> Result<String, JsValue> {
    let indent = whole(
        indent,
        2,
        "indent must be a whole number of spaces between 0 (minified) and 8",
    )?;
    let skip = whole(skip, 0, "skip must be a whole number of records, 0 or more")?;
    let limit = whole(
        limit,
        0,
        "limit must be a whole number of records, 0 (show every match) or more",
    )?;

    gizza_ai_ndjson_viewer_core::run(
        data,
        or_default(view, "pretty"),
        indent,
        search,
        or_default(search_in, "any"),
        truthy(case_sensitive),
        path,
        value,
        or_default(match_mode, "contains"),
        truthy(sort_keys),
        skip,
        limit,
        or_default(invalid, "report"),
        truthy(line_numbers),
        truthy(stats),
    )
    .map_err(|e| JsValue::from_str(&e))
}
