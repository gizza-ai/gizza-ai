//! Browser-facing wasm-bindgen wrapper for /tools/csv-header-sanitizer/.
//! Field order MUST match page/meta.toml: data, delimiter, style, ascii,
//! leading_digit, max_length, blank_name, dedupe, output. Every field arrives as
//! a string (checkboxes send "true"/"false"); the core owns all validation and
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

/// Sanitize the header row of a CSV table.
///
/// - `data`: the CSV/TSV table text; row 1 is the header.
/// - `delimiter`: `auto`, a single character, or `comma`/`tab`/`semicolon`/`pipe`.
/// - `style`: `snake` | `camel` | `pascal` | `kebab` | `screaming_snake` |
///   `lower` | `preserve`.
/// - `ascii`: checkbox `"true"`/`"false"` (default-checked) — transliterate
///   Unicode to ASCII.
/// - `leading_digit`: `underscore` | `col` | `keep`.
/// - `max_length`: a character cap (blank/unparseable → `0` = no limit).
/// - `blank_name`: base name for blank headers (blank → `column`).
/// - `dedupe`: `suffix` | `index` | `allow`.
/// - `output`: `csv` | `header` | `mapping`.
///
/// Throws a JS error string on empty input or an unknown option.
#[allow(clippy::too_many_arguments)]
#[wasm_bindgen]
pub fn run(
    data: &str,
    delimiter: &str,
    style: &str,
    ascii: &str,
    leading_digit: &str,
    max_length: &str,
    blank_name: &str,
    dedupe: &str,
    output: &str,
) -> Result<String, JsValue> {
    let max_length = max_length.trim().parse::<u32>().unwrap_or(0);
    gizza_ai_csv_header_sanitizer_core::sanitize(
        data,
        delimiter,
        style,
        truthy(ascii),
        leading_digit,
        max_length,
        blank_name,
        dedupe,
        output,
    )
    .map_err(|e| JsValue::from_str(&e))
}
