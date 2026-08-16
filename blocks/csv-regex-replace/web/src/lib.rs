//! Browser-facing wasm-bindgen wrapper for /tools/csv-regex-replace/.
//! Field order MUST match page/meta.toml: data, pattern, replacement, columns,
//! mode, match_scope, ignore_case, multiline, dotall, replace_all, has_header,
//! include_header, delimiter, quote_style, output. Every field arrives as a
//! string (checkboxes send "true"/"false"); the core owns all validation and
//! error messages.
use wasm_bindgen::prelude::*;

/// `"true"`/`"1"`/`"yes"`/`"on"` (case-insensitive) → `true`; anything else
/// (including blank) → `false`. Checkboxes on the page send `"true"`/`"false"`.
fn truthy(s: &str) -> bool {
    matches!(
        s.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "on" | "yes"
    )
}

/// Find-and-replace inside the chosen columns of a CSV table.
///
/// - `data`: the CSV/TSV table text (max 5,000,000 bytes).
/// - `pattern`: a regular expression, or literal text under `mode = "literal"`.
/// - `replacement`: the replacement text; `$1`/`${name}`/`$0`/`$$` expand in
///   regex mode, blank deletes every match.
/// - `columns`: blank (or `*`) for every column, else names / 1-based indices /
///   `2-4` ranges, comma-separated.
/// - `mode`: `regex` | `literal`.
/// - `match_scope`: `substring` | `whole_cell`.
/// - `ignore_case`, `multiline`, `dotall`: checkbox `"true"`/`"false"` — the
///   regex `i`, `m` and `s` flags (all default-off).
/// - `replace_all`: checkbox `"true"`/`"false"` (default-checked).
/// - `has_header`: checkbox `"true"`/`"false"` (default-checked).
/// - `include_header`: checkbox `"true"`/`"false"` (default-unchecked).
/// - `delimiter`: `auto`, a single character, or `comma`/`tab`/`semicolon`/`pipe`.
/// - `quote_style`: `minimal` | `always` | `non_numeric`.
/// - `output`: `csv` | `changed` | `report`.
///
/// Throws a JS error string on empty input, an empty or invalid pattern, an
/// unknown option, an unknown column, or an over-cap table.
#[allow(clippy::too_many_arguments)]
#[wasm_bindgen]
pub fn run(
    data: &str,
    pattern: &str,
    replacement: &str,
    columns: &str,
    mode: &str,
    match_scope: &str,
    ignore_case: &str,
    multiline: &str,
    dotall: &str,
    replace_all: &str,
    has_header: &str,
    include_header: &str,
    delimiter: &str,
    quote_style: &str,
    output: &str,
) -> Result<String, JsValue> {
    gizza_ai_csv_regex_replace_core::replace(
        data,
        pattern,
        replacement,
        columns,
        mode,
        match_scope,
        truthy(ignore_case),
        truthy(multiline),
        truthy(dotall),
        truthy(replace_all),
        truthy(has_header),
        truthy(include_header),
        delimiter,
        quote_style,
        output,
    )
    .map_err(|e| JsValue::from_str(&e))
}
