//! Browser-facing wasm-bindgen wrapper for /tools/iqr-outlier-trimmer/.
//! Field order MUST match page/meta.toml: data, columns, k, action, output,
//! header, delimiter, quartile_method, match_mode, non_numeric. Every field
//! arrives as a string (checkboxes send "true"/"false").
use gizza_ai_iqr_outlier_trimmer_core::trim;
use wasm_bindgen::prelude::*;

fn truthy(s: &str) -> bool {
    matches!(s.trim().to_ascii_lowercase().as_str(), "true" | "1" | "on" | "yes")
}

fn or(value: &str, default: &'static str) -> String {
    let t = value.trim();
    if t.is_empty() { default.to_string() } else { t.to_string() }
}

#[wasm_bindgen]
#[allow(clippy::too_many_arguments)]
pub fn run(
    data: &str,
    columns: &str,
    k: &str,
    action: &str,
    output: &str,
    header: &str,
    delimiter: &str,
    quartile_method: &str,
    match_mode: &str,
    non_numeric: &str,
) -> Result<String, JsValue> {
    let kv = match k.trim() {
        "" => 1.5,
        t => t.parse::<f64>().map_err(|_| JsValue::from_str("k must be a number >= 0"))?,
    };
    // A checkbox that never rendered arrives empty; `header` defaults to true.
    let has_header = header.trim().is_empty() || truthy(header);
    trim(
        data,
        columns,
        kv,
        &or(action, "remove"),
        &or(output, "csv"),
        has_header,
        &or(delimiter, "comma"),
        &or(quartile_method, "linear"),
        &or(match_mode, "any"),
        &or(non_numeric, "keep"),
    )
    .map_err(|e| JsValue::from_str(&e))
}
