//! Browser-facing wasm-bindgen wrapper for /tools/regex-capture-to-csv/.
//! Field order MUST match page/meta.toml: text, pattern, columns, delimiter,
//! header, quoting, line_ending, ignore_case, multiline, dotall, unique, sort.
//! Fields arrive as strings.
use gizza_ai_regex_capture_to_csv_core::to_csv;
use wasm_bindgen::prelude::*;

fn truthy(v: &str) -> bool {
    matches!(
        v.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "on" | "yes"
    )
}

#[wasm_bindgen]
#[allow(clippy::too_many_arguments)]
pub fn run(
    text: &str,
    pattern: &str,
    columns: &str,
    delimiter: &str,
    header: &str,
    quoting: &str,
    line_ending: &str,
    ignore_case: &str,
    multiline: &str,
    dotall: &str,
    unique: &str,
    sort: &str,
) -> Result<String, JsValue> {
    to_csv(
        text,
        pattern,
        columns,
        delimiter,
        truthy(header),
        quoting,
        line_ending,
        truthy(ignore_case),
        truthy(multiline),
        truthy(dotall),
        truthy(unique),
        truthy(sort),
    )
    .map_err(|e| JsValue::from_str(&e))
}
