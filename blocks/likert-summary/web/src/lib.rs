//! Browser-facing wasm-bindgen wrapper for /tools/likert-summary/.
//! Field order MUST match page/meta.toml: data, input, items, points, scale,
//! labels, reverse, box_size, missing, sort, decimals, chart, diverging, alpha,
//! delimiter. Each value arrives as a string (checkboxes send "true"/"false");
//! blank numeric fields fall back to the documented defaults.
use wasm_bindgen::prelude::*;

fn truthy(s: &str) -> bool {
    matches!(s.trim().to_ascii_lowercase().as_str(), "true" | "1" | "on" | "yes")
}

/// Parse an optional whole-number field: blank → `fallback`.
fn parse_int(s: &str, field: &str, fallback: i64) -> Result<i64, JsValue> {
    let t = s.trim();
    if t.is_empty() {
        return Ok(fallback);
    }
    t.parse::<i64>()
        .map_err(|_| JsValue::from_str(&format!("{field} must be a whole number (got '{t}')")))
}

#[allow(clippy::too_many_arguments)]
#[wasm_bindgen]
pub fn run(
    data: &str,
    input: &str,
    items: &str,
    points: &str,
    scale: &str,
    labels: &str,
    reverse: &str,
    box_size: &str,
    missing: &str,
    sort: &str,
    decimals: &str,
    chart: &str,
    diverging: &str,
    alpha: &str,
    delimiter: &str,
) -> Result<String, JsValue> {
    let points = parse_int(points, "points", 5)?;
    let box_size = parse_int(box_size, "box_size", 2)?;
    let decimals = parse_int(decimals, "decimals", 2)?;
    gizza_ai_likert_summary_core::summarize(
        data,
        input,
        items,
        points,
        scale,
        labels,
        reverse,
        box_size,
        missing,
        sort,
        decimals,
        truthy(chart),
        truthy(diverging),
        truthy(alpha),
        delimiter,
    )
    .map_err(|e| JsValue::from_str(&e))
}
