//! Browser-facing wasm-bindgen wrapper for /tools/csv-column-mapping-suggest/.
use gizza_ai_csv_column_mapping_suggest_core::{run as core_run, Delimiter, Options, OutputFormat};
use wasm_bindgen::prelude::*;

fn truthy(v: &str) -> bool {
    matches!(
        v.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "on" | "yes"
    )
}

#[wasm_bindgen]
pub fn run(
    source: &str,
    target: &str,
    delimiter: &str,
    header: &str,
    sample_rows: &str,
    header_weight: &str,
    threshold: &str,
    format: &str,
) -> Result<String, JsValue> {
    let opts = Options {
        delimiter: Delimiter::parse(delimiter),
        header: truthy(header),
        sample_rows: sample_rows.trim().parse::<usize>().unwrap_or(50).min(500),
        header_weight: header_weight.trim().parse::<f64>().unwrap_or(0.6),
        threshold: threshold.trim().parse::<f64>().unwrap_or(0.3),
        format: OutputFormat::parse(format),
    };
    core_run(source, target, &opts).map_err(|e| JsValue::from_str(&e))
}
