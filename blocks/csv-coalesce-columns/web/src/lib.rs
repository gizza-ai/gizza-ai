//! Browser-facing wasm-bindgen wrapper for /tools/csv-coalesce-columns/.
//! The page hands every field over as a raw string, so booleans are parsed here:
//! default-false flags need a positive truthy value, default-true flags stay on
//! unless explicitly switched off.
use gizza_ai_csv_coalesce_columns_core::coalesce_columns;
use wasm_bindgen::prelude::*;

fn truthy(v: &str) -> bool {
    matches!(
        v.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "on" | "yes"
    )
}

fn not_falsy(v: &str) -> bool {
    !matches!(
        v.trim().to_ascii_lowercase().as_str(),
        "false" | "0" | "off" | "no"
    )
}

#[wasm_bindgen]
#[allow(clippy::too_many_arguments)]
pub fn run(
    data: &str,
    columns: &str,
    output: &str,
    position: &str,
    fallback: &str,
    drop_sources: &str,
    blank_is_empty: &str,
    null_tokens: &str,
    header: &str,
    delimiter: &str,
) -> Result<String, JsValue> {
    let delim = if delimiter.is_empty() { "," } else { delimiter };
    coalesce_columns(
        data,
        columns,
        output,
        position,
        fallback,
        truthy(drop_sources),
        not_falsy(blank_is_empty),
        null_tokens,
        not_falsy(header),
        delim,
    )
    .map_err(|e| JsValue::from_str(&e))
}
