//! Browser-facing wasm-bindgen wrapper for /tools/numeric-row-deduplicator/.
//! The page passes every field value as a raw string, so parse them here and
//! funnel through the shared core (which owns all validation).
use gizza_ai_numeric_row_deduplicator_core::dedupe_numeric;
use wasm_bindgen::prelude::*;

fn truthy(v: &str) -> bool {
    matches!(v.trim().to_ascii_lowercase().as_str(), "true" | "1" | "on" | "yes")
}

#[wasm_bindgen]
pub fn run(
    data: &str,
    columns: &str,
    header: &str,
    delimiter: &str,
    precision: &str,
    keep: &str,
) -> Result<String, JsValue> {
    let prec: i64 = if precision.trim().is_empty() {
        -1
    } else {
        precision
            .trim()
            .parse()
            .map_err(|_| JsValue::from_str("precision must be a whole number between -1 and 12"))?
    };
    let delim = if delimiter.is_empty() { "," } else { delimiter };
    let keep = if keep.is_empty() { "first" } else { keep };
    dedupe_numeric(data, columns, truthy(header), delim, prec, keep)
        .map_err(|e| JsValue::from_str(&e))
}
