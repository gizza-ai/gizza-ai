//! Browser-facing wasm-bindgen wrapper for /tools/csv-null-standardizer/.
//! Field order MUST match page/meta.toml: input, delimiter, na_tokens,
//! replace_with, blank_is_missing, case_sensitive, trim, header, columns,
//! quote_style. Every field arrives as a string (checkboxes send "true"/"false");
//! the core owns all validation and error messages.
use wasm_bindgen::prelude::*;

fn truthy(s: &str) -> bool {
    matches!(
        s.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "on" | "yes"
    )
}

/// Rewrite every missing-value token in a CSV table to one representation.
///
/// - `input`: the CSV/TSV table text.
/// - `delimiter`: `auto`, a single character, or `comma`/`tab`/`semicolon`/`pipe`.
/// - `na_tokens`: comma-separated tokens that count as missing (empty → blanks only).
/// - `replace_with`: what every missing cell becomes (empty → a blank cell).
/// - `blank_is_missing` / `case_sensitive` / `trim` / `header`: checkbox
///   `"true"`/`"false"`.
/// - `columns`: comma-separated column names or 1-based positions (empty → all).
/// - `quote_style`: `minimal` | `always` | `never`.
///
/// Throws a JS error string on invalid input or an unknown option.
#[allow(clippy::too_many_arguments)]
#[wasm_bindgen]
pub fn run(
    input: &str,
    delimiter: &str,
    na_tokens: &str,
    replace_with: &str,
    blank_is_missing: &str,
    case_sensitive: &str,
    trim: &str,
    header: &str,
    columns: &str,
    quote_style: &str,
) -> Result<String, JsValue> {
    gizza_ai_csv_null_standardizer_core::standardize(
        input,
        delimiter,
        na_tokens,
        replace_with,
        truthy(blank_is_missing),
        truthy(case_sensitive),
        truthy(trim),
        truthy(header),
        columns,
        quote_style,
    )
    .map_err(|e| JsValue::from_str(&e))
}
