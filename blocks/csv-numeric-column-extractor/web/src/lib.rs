//! Browser-facing wasm-bindgen wrapper for /tools/csv-numeric-column-extractor/.
//! Field order MUST match meta.toml: data, delimiter, header, output, null_tokens,
//! allow_blanks, min_numeric_ratio, normalize. Fields arrive as strings (checkboxes
//! send "true"/"false"); empty values fall back to the schema defaults.
use gizza_ai_csv_numeric_column_extractor_core::DEFAULT_NULL_TOKENS;
use wasm_bindgen::prelude::*;

/// `allow_blanks` and `normalize` both default to true, so a blank value (the field
/// missing from the query string) also means "on".
fn truthy(s: &str) -> bool {
    matches!(
        s.trim().to_ascii_lowercase().as_str(),
        "" | "true" | "1" | "on" | "yes"
    )
}

#[wasm_bindgen]
pub fn run(
    data: &str,
    delimiter: &str,
    header: &str,
    output: &str,
    null_tokens: &str,
    allow_blanks: &str,
    min_numeric_ratio: &str,
    normalize: &str,
) -> Result<String, JsValue> {
    let null_tokens = if null_tokens.trim().is_empty() {
        DEFAULT_NULL_TOKENS
    } else {
        null_tokens
    };
    let ratio = if min_numeric_ratio.trim().is_empty() {
        1.0
    } else {
        min_numeric_ratio.trim().parse::<f64>().map_err(|_| {
            JsValue::from_str(&format!(
                "min_numeric_ratio must be a number between 0.1 and 1.0, got '{}'",
                min_numeric_ratio.trim()
            ))
        })?
    };
    gizza_ai_csv_numeric_column_extractor_core::extract(
        data,
        delimiter,
        header,
        output,
        null_tokens,
        truthy(allow_blanks),
        ratio,
        truthy(normalize),
    )
    .map_err(|e| JsValue::from_str(&e))
}
