//! Browser-facing wasm-bindgen wrapper for /tools/csv-chart-generator/.
//! The page passes every field as a string (in declared meta.toml order); this
//! parses the numeric options, builds the core Options, and returns the SVG.
use gizza_ai_csv_chart_generator_core::{render, Options};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(
    data: &str,
    chart_type: &str,
    x_column: &str,
    y_column: &str,
    title: &str,
    width: &str,
    height: &str,
    color: &str,
    bins: &str,
) -> Result<String, JsValue> {
    // Empty input → empty result (the page shows a neutral idle state rather
    // than a red error on first load / after Reset).
    if data.trim().is_empty() {
        return Ok(String::new());
    }
    let opts = Options {
        chart_type: if chart_type.trim().is_empty() { "bar".to_string() } else { chart_type.to_string() },
        x_column: x_column.to_string(),
        y_column: y_column.to_string(),
        title: title.to_string(),
        width: width.trim().parse::<u32>().unwrap_or(800),
        height: height.trim().parse::<u32>().unwrap_or(400),
        color: if color.trim().is_empty() { "#2563eb".to_string() } else { color.to_string() },
        bins: bins.trim().parse::<u32>().unwrap_or(10),
    };
    render(data, &opts).map_err(|e| JsValue::from_str(&e))
}
