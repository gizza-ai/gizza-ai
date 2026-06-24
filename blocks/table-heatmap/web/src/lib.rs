//! Browser-facing wasm-bindgen wrapper for /tools/table-heatmap/.
//! Field order MUST match meta.toml: data, scale, header, per_column, min, midpoint, max, delimiter.
use gizza_ai_table_heatmap_core::{heatmap, Bounds, Scale};
use wasm_bindgen::prelude::*;

fn truthy(v: &str) -> bool {
    matches!(v.trim().to_ascii_lowercase().as_str(), "true" | "1" | "on" | "yes")
}

fn opt_num(v: &str) -> Result<Option<f64>, JsValue> {
    let t = v.trim();
    if t.is_empty() {
        return Ok(None);
    }
    t.parse::<f64>()
        .map(Some)
        .map_err(|_| JsValue::from_str(&format!("not a number: '{t}'")))
}

#[wasm_bindgen]
#[allow(clippy::too_many_arguments)]
pub fn run(
    data: &str,
    scale: &str,
    header: &str,
    per_column: &str,
    min: &str,
    midpoint: &str,
    max: &str,
    delimiter: &str,
) -> Result<String, JsValue> {
    let sc = Scale::parse(scale).map_err(|e| JsValue::from_str(&e))?;
    let delim = if delimiter.is_empty() { "," } else { delimiter };
    let bounds = Bounds {
        min: opt_num(min)?,
        midpoint: opt_num(midpoint)?,
        max: opt_num(max)?,
    };
    heatmap(data, sc, truthy(header), truthy(per_column), bounds, delim).map_err(|e| JsValue::from_str(&e))
}
