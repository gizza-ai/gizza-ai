//! Browser-facing wasm-bindgen wrapper for /tools/data-normalize/.
//! Field order MUST match meta.toml: input, method, header, delimiter, columns,
//! range_min, range_max, ddof, with_centering, with_scaling. Fields arrive as
//! strings (checkboxes send "true"/"false").
use gizza_ai_data_normalize_core::normalize;
use wasm_bindgen::prelude::*;

fn truthy(s: &str) -> bool {
    matches!(s.trim().to_ascii_lowercase().as_str(), "true" | "1" | "on" | "yes")
}

fn parse_f64(s: &str, default: f64, label: &str) -> Result<f64, JsValue> {
    let t = s.trim();
    if t.is_empty() {
        return Ok(default);
    }
    t.parse::<f64>()
        .map_err(|_| JsValue::from_str(&format!("{label} must be a number")))
}

fn parse_ddof(s: &str) -> Result<u32, JsValue> {
    let t = s.trim();
    if t.is_empty() {
        return Ok(0);
    }
    t.parse::<u32>()
        .map_err(|_| JsValue::from_str("ddof must be 0 or 1"))
}

#[wasm_bindgen]
#[allow(clippy::too_many_arguments)]
pub fn run(
    input: &str,
    method: &str,
    header: &str,
    delimiter: &str,
    columns: &str,
    range_min: &str,
    range_max: &str,
    ddof: &str,
    with_centering: &str,
    with_scaling: &str,
) -> Result<String, JsValue> {
    let m = if method.is_empty() { "min_max" } else { method };
    let delim = if delimiter.is_empty() { "comma" } else { delimiter };
    let rmin = parse_f64(range_min, 0.0, "range_min")?;
    let rmax = parse_f64(range_max, 1.0, "range_max")?;
    let dd = parse_ddof(ddof)?;
    // Checkboxes only POST when checked; treat an absent (empty) value as the
    // documented default of true.
    let centering = with_centering.trim().is_empty() || truthy(with_centering);
    let scaling = with_scaling.trim().is_empty() || truthy(with_scaling);
    normalize(input, truthy(header), delim, m, columns, rmin, rmax, dd, centering, scaling)
        .map_err(|e| JsValue::from_str(&e))
}
