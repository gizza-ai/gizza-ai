//! Browser-facing wasm-bindgen wrapper for /tools/csv-type-inferrer/.
//! Field order MUST match meta.toml: data, delimiter, headers, null_tokens,
//! date_detection, output. Fields arrive as strings (the checkbox sends
//! "true"/"false"); an empty null_tokens field falls back to the schema default.
use wasm_bindgen::prelude::*;

const DEFAULT_NULL_TOKENS: &str = "NA,N/A,NULL,null,None,nan";

fn truthy(s: &str) -> bool {
    // date_detection defaults to true, so a blank value (no checkbox in the
    // query string) also means "on".
    matches!(
        s.trim().to_ascii_lowercase().as_str(),
        "" | "true" | "1" | "on" | "yes"
    )
}

#[wasm_bindgen]
pub fn run(
    data: &str,
    delimiter: &str,
    headers: &str,
    null_tokens: &str,
    date_detection: &str,
    output: &str,
) -> Result<String, JsValue> {
    let null_tokens = if null_tokens.trim().is_empty() {
        DEFAULT_NULL_TOKENS
    } else {
        null_tokens
    };
    gizza_ai_csv_type_inferrer_core::infer(
        data,
        delimiter,
        headers,
        null_tokens,
        truthy(date_detection),
        output,
    )
    .map_err(|e| JsValue::from_str(&e))
}
