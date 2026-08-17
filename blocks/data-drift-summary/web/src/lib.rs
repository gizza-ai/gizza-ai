//! Browser-facing wasm-bindgen wrapper for /tools/data-drift-summary/.
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
#[allow(clippy::too_many_arguments)]
pub fn run(
    reference: &str,
    current: &str,
    delimiter: &str,
    header: &str,
    columns: &str,
    method: &str,
    threshold: &str,
    bins: &str,
    max_categories: &str,
    drift_share: &str,
    ignore_case: &str,
    sort: &str,
    format: &str,
) -> Result<String, JsValue> {
    // `header` defaults ON (checked); `ignore_case` defaults OFF.
    let header = truthy_default(header, true);
    let ignore_case = truthy_default(ignore_case, false);
    let threshold = num_field(threshold, 0.2, "threshold")?;
    let drift_share = num_field(drift_share, 0.5, "drift share")?;
    let bins = num_field(bins, 10.0, "bins")? as usize;
    let max_categories = num_field(max_categories, 20.0, "max categories")? as usize;
    gizza_ai_data_drift_summary_core::run(
        reference,
        current,
        delimiter,
        header,
        columns,
        method,
        threshold,
        bins,
        max_categories,
        drift_share,
        ignore_case,
        sort,
        format,
    )
    .map_err(|e| JsValue::from_str(&e))
}

/// Parse a checkbox/flag value; empty falls back to `default`.
fn truthy_default(s: &str, default: bool) -> bool {
    match s.trim().to_ascii_lowercase().as_str() {
        "" => default,
        "true" | "1" | "yes" | "on" => true,
        _ => false,
    }
}

/// Parse a numeric field; empty falls back to `default`, anything unparseable
/// reports what was expected rather than silently using the default.
fn num_field(s: &str, default: f64, label: &str) -> Result<f64, JsValue> {
    let t = s.trim();
    if t.is_empty() {
        return Ok(default);
    }
    t.parse::<f64>()
        .ok()
        .filter(|v| v.is_finite())
        .ok_or_else(|| JsValue::from_str(&format!("{label} must be a number, got '{t}'")))
}
