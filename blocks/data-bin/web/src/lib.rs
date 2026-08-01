//! Browser-facing wasm-bindgen wrapper for /tools/data-bin/.
//! Field order MUST match meta.toml: input, method, column, bins, edges, labels,
//! label_style, right, precision, output, header, delimiter. Fields arrive as
//! strings (checkboxes send "true"/"false").
use gizza_ai_data_bin_core::bin;
use wasm_bindgen::prelude::*;

fn truthy(s: &str) -> bool {
    matches!(s.trim().to_ascii_lowercase().as_str(), "true" | "1" | "on" | "yes")
}

fn parse_u32(s: &str, default: u32, label: &str) -> Result<u32, JsValue> {
    let t = s.trim();
    if t.is_empty() {
        return Ok(default);
    }
    t.parse::<u32>()
        .map_err(|_| JsValue::from_str(&format!("{label} must be a non-negative whole number")))
}

#[wasm_bindgen]
#[allow(clippy::too_many_arguments)]
pub fn run(
    input: &str,
    method: &str,
    column: &str,
    bins: &str,
    edges: &str,
    labels: &str,
    label_style: &str,
    right: &str,
    precision: &str,
    output: &str,
    header: &str,
    delimiter: &str,
) -> Result<String, JsValue> {
    let m = if method.is_empty() { "equal_width" } else { method };
    let style = if label_style.is_empty() { "range" } else { label_style };
    let out = if output.is_empty() { "append" } else { output };
    let delim = if delimiter.is_empty() { "comma" } else { delimiter };
    let nbins = parse_u32(bins, 4, "bins")?;
    let prec = parse_u32(precision, 3, "precision")?;
    // Checkboxes only POST when checked; treat an absent (empty) value as the
    // documented default of true.
    let right_closed = right.trim().is_empty() || truthy(right);
    let has_header = header.trim().is_empty() || truthy(header);
    bin(
        input,
        has_header,
        delim,
        column,
        m,
        nbins,
        edges,
        labels,
        style,
        right_closed,
        prec,
        out,
    )
    .map_err(|e| JsValue::from_str(&e))
}
