//! Browser-facing wasm-bindgen wrapper for /tools/document-term-matrix/.
use wasm_bindgen::prelude::*;

fn flag(s: &str) -> bool {
    matches!(
        s.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "on" | "yes"
    )
}

fn whole(v: f64, default: u32, name: &str, max: u32) -> Result<u32, String> {
    if !v.is_finite() {
        return Ok(default);
    }
    if v < 0.0 || v.fract() != 0.0 {
        return Err(format!("{name} must be a whole number (got {v})"));
    }
    let n = v as u32;
    if n > max {
        return Err(format!("{name} must be at most {max} (got {n})"));
    }
    Ok(n)
}

#[allow(clippy::too_many_arguments)]
#[wasm_bindgen]
pub fn run(
    documents: &str,
    input_format: &str,
    weighting: &str,
    case_sensitive: &str,
    ngram_min: f64,
    ngram_max: f64,
    min_df: f64,
    max_features: f64,
    output: &str,
    include_totals: &str,
) -> Result<String, JsValue> {
    let ngram_min = whole(ngram_min, 1, "ngram_min", 3).map_err(|e| JsValue::from_str(&e))?;
    let ngram_max = whole(ngram_max, 1, "ngram_max", 3).map_err(|e| JsValue::from_str(&e))?;
    let min_df = whole(min_df, 1, "min_df", 100_000).map_err(|e| JsValue::from_str(&e))?;
    let max_features =
        whole(max_features, 0, "max_features", 5_000).map_err(|e| JsValue::from_str(&e))?;
    gizza_ai_document_term_matrix_core::run(
        documents,
        if input_format.trim().is_empty() {
            "auto"
        } else {
            input_format
        },
        if weighting.trim().is_empty() {
            "count"
        } else {
            weighting
        },
        flag(case_sensitive),
        ngram_min,
        ngram_max,
        min_df,
        max_features,
        if output.trim().is_empty() {
            "csv"
        } else {
            output
        },
        flag(include_totals),
    )
    .map_err(|e| JsValue::from_str(&e))
}
