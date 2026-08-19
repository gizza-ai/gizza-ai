//! Browser-facing wasm-bindgen wrapper for /tools/csv-whitespace-normalizer/.
//! Field order MUST match page/meta.toml: input, delimiter, trim, internal,
//! whitespace, columns, header, normalize_header. Every field arrives as a string
//! (checkboxes send "true"/"false"); the core owns all validation and error
//! messages.
use wasm_bindgen::prelude::*;

fn truthy(s: &str) -> bool {
    matches!(
        s.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "on" | "yes"
    )
}

/// Normalize the whitespace inside every cell of a CSV/delimited table.
///
/// - `input`: the CSV/TSV table text.
/// - `delimiter`: `auto`, a single character, or `comma`/`tab`/`semicolon`/`pipe`.
/// - `trim`: `both` | `leading` | `trailing` | `none`.
/// - `internal`: `collapse` | `remove` | `keep`.
/// - `whitespace`: `unicode` (includes NBSP and friends) | `ascii`.
/// - `columns`: comma-separated names, 1-based positions and `2-4` ranges (empty → all).
/// - `header` / `normalize_header`: checkbox `"true"`/`"false"`.
///
/// Throws a JS error string on invalid input or an unknown option.
#[allow(clippy::too_many_arguments)]
#[wasm_bindgen]
pub fn run(
    input: &str,
    delimiter: &str,
    trim: &str,
    internal: &str,
    whitespace: &str,
    columns: &str,
    header: &str,
    normalize_header: &str,
) -> Result<String, JsValue> {
    gizza_ai_csv_whitespace_normalizer_core::normalize(
        input,
        delimiter,
        trim,
        internal,
        whitespace,
        columns,
        truthy(header),
        truthy(normalize_header),
    )
    .map_err(|e| JsValue::from_str(&e))
}
