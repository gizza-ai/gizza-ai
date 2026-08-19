//! Browser-facing wasm-bindgen wrapper for /tools/constant-column-dropper/.
use wasm_bindgen::prelude::*;

fn truthy(v: &str, default: bool) -> bool {
    let s = v.trim().to_ascii_lowercase();
    if s.is_empty() {
        default
    } else {
        matches!(s.as_str(), "true" | "1" | "on" | "yes")
    }
}

/// Parse a numeric field, falling back to `default` when blank.
fn num(v: &str, default: f64) -> Result<f64, JsValue> {
    let s = v.trim();
    if s.is_empty() {
        return Ok(default);
    }
    s.parse::<f64>()
        .map_err(|_| JsValue::from_str(&format!("dominance must be a number, got '{s}'")))
}

#[allow(clippy::too_many_arguments)]
#[wasm_bindgen]
pub fn run(
    data: &str,
    header: &str,
    delimiter: &str,
    dominance: &str,
    empty_cells: &str,
    ignore_case: &str,
    ignore_whitespace: &str,
    keep: &str,
    output: &str,
) -> Result<String, JsValue> {
    gizza_ai_constant_column_dropper_core::drop_constant(
        data,
        truthy(header, true),
        delimiter,
        num(dominance, 100.0)?,
        empty_cells,
        truthy(ignore_case, true),
        truthy(ignore_whitespace, true),
        keep,
        output,
    )
    .map_err(|e| JsValue::from_str(&e))
}
