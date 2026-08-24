//! Browser-facing wasm-bindgen wrapper for /tools/csv-column-number-formatter/.
//! Field order MUST match page/meta.toml: data, columns, decimals, rounding,
//! notation, grouping, group_separator, decimal_separator, sign, prefix,
//! suffix, input_decimal, non_numeric, has_header, delimiter, quote_style,
//! output. Every field arrives as a string (checkboxes send "true"/"false");
//! the core owns all validation and error messages.
use wasm_bindgen::prelude::*;

/// `"true"`/`"1"`/`"yes"`/`"on"` (case-insensitive) → `true`; anything else
/// (including blank) → `false`. Checkboxes on the page send `"true"`/`"false"`.
fn truthy(s: &str) -> bool {
    matches!(
        s.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "on" | "yes"
    )
}

/// Apply one uniform numeric format to the chosen columns of a CSV table.
///
/// - `data`: the CSV/TSV table text (max 5,000,000 bytes).
/// - `columns`: blank (or `*`) for every column, else names / 1-based indices /
///   `2-4` ranges, comma-separated.
/// - `decimals`: fractional digits to keep, `-9`..=`15`; negative rounds to
///   tens/hundreds/thousands. Blank falls back to `2`.
/// - `rounding`: `half_up` | `half_down` | `half_even` | `ceil` | `floor` | `truncate`.
/// - `notation`: `standard` | `compact` | `scientific` | `percent`.
/// - `grouping`: `none` | `thousands` | `indian`.
/// - `group_separator`: `comma` | `period` | `space` | `thin_space` | `apostrophe` | `underscore`.
/// - `decimal_separator`: `period` | `comma`.
/// - `sign`: `auto` | `always` | `except_zero` | `never` | `space` | `parens`.
/// - `prefix` / `suffix`: text wrapped around each formatted number.
/// - `input_decimal`: `auto` | `dot` | `comma`.
/// - `non_numeric`: `keep` | `blank` | `error`.
/// - `has_header`: checkbox `"true"`/`"false"` (default-checked).
/// - `delimiter`: `auto`, a single character, or `comma`/`tab`/`semicolon`/`pipe`.
/// - `quote_style`: `minimal` | `always` | `non_numeric`.
/// - `output`: `csv` | `changed` | `report`.
///
/// Throws a JS error string on empty input, a non-integer or out-of-range
/// `decimals`, an unknown option, an unknown column, a non-numeric cell under
/// `non_numeric = "error"`, or an over-cap table.
#[allow(clippy::too_many_arguments)]
#[wasm_bindgen]
pub fn run(
    data: &str,
    columns: &str,
    decimals: &str,
    rounding: &str,
    notation: &str,
    grouping: &str,
    group_separator: &str,
    decimal_separator: &str,
    sign: &str,
    prefix: &str,
    suffix: &str,
    input_decimal: &str,
    non_numeric: &str,
    has_header: &str,
    delimiter: &str,
    quote_style: &str,
    output: &str,
) -> Result<String, JsValue> {
    let trimmed = decimals.trim();
    let decimals: i32 = if trimmed.is_empty() {
        2
    } else {
        trimmed.parse().map_err(|_| {
            JsValue::from_str(&format!(
                "decimals must be a whole number between -9 and 15, got '{trimmed}'"
            ))
        })?
    };
    gizza_ai_csv_column_number_formatter_core::format_columns(
        data,
        columns,
        decimals,
        rounding,
        notation,
        grouping,
        group_separator,
        decimal_separator,
        sign,
        prefix,
        suffix,
        input_decimal,
        non_numeric,
        truthy(has_header),
        delimiter,
        quote_style,
        output,
    )
    .map_err(|e| JsValue::from_str(&e))
}
