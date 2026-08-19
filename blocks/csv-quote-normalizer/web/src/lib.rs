//! Browser-facing wasm-bindgen wrapper for /tools/csv-quote-normalizer/.
//! Argument order MUST match page/meta.toml: input, delimiter, output_delimiter,
//! input_quote, quote_style, output_quote, escape, backslash_escapes,
//! smart_quotes, line_ending, output. Every field arrives as a string (checkboxes
//! send "true"/"false"); the core owns all validation and error messages.
use wasm_bindgen::prelude::*;

fn truthy(s: &str) -> bool {
    matches!(
        s.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "on" | "yes"
    )
}

/// Re-emit a CSV with one consistent quoting/escaping/delimiter dialect.
///
/// - `input`: the CSV text (read with a tolerant parser).
/// - `delimiter`: `auto`, a single character, or `comma`/`tab`/`semicolon`/`pipe`/`space`.
/// - `output_delimiter`: `same` or a delimiter spec.
/// - `input_quote`: `auto` | `double` | `single` | `none`.
/// - `quote_style`: `minimal` | `always` | `non_numeric` | `never`.
/// - `output_quote`: `double` | `single`.
/// - `escape`: `doubled` (RFC 4180 `""`) | `backslash` (`\"`).
/// - `backslash_escapes` / `smart_quotes`: checkbox `"true"`/`"false"`.
/// - `line_ending`: `lf` | `crlf`.
/// - `output`: `csv` | `report`.
///
/// Throws a JS error string on invalid input or an unknown option.
#[allow(clippy::too_many_arguments)]
#[wasm_bindgen]
pub fn run(
    input: &str,
    delimiter: &str,
    output_delimiter: &str,
    input_quote: &str,
    quote_style: &str,
    output_quote: &str,
    escape: &str,
    backslash_escapes: &str,
    smart_quotes: &str,
    line_ending: &str,
    output: &str,
) -> Result<String, JsValue> {
    gizza_ai_csv_quote_normalizer_core::normalize(
        input,
        delimiter,
        output_delimiter,
        input_quote,
        quote_style,
        output_quote,
        escape,
        truthy(backslash_escapes),
        truthy(smart_quotes),
        line_ending,
        output,
    )
    .map_err(|e| JsValue::from_str(&e))
}
