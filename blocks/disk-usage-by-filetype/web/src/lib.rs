//! Browser-facing wasm-bindgen wrapper for /tools/disk-usage-by-filetype/.
use wasm_bindgen::prelude::*;

use gizza_ai_disk_usage_by_filetype_core::Options;

/// Page checkboxes arrive as "true"/"false"; be liberal about the truthy forms.
fn boolish(s: &str, fallback: bool) -> bool {
    match s.trim().to_ascii_lowercase().as_str() {
        "" => fallback,
        v => matches!(v, "true" | "1" | "on" | "yes"),
    }
}

fn text_or(s: &str, fallback: &str) -> String {
    if s.trim().is_empty() {
        fallback.into()
    } else {
        s.trim().into()
    }
}

fn parse_u32(name: &str, s: &str, fallback: u32) -> Result<u32, JsValue> {
    let t = s.trim();
    if t.is_empty() {
        return Ok(fallback);
    }
    t.parse::<f64>()
        .ok()
        .filter(|v| v.is_finite() && *v >= 0.0 && *v <= u32::MAX as f64)
        .map(|v| v.round() as u32)
        .ok_or_else(|| JsValue::from_str(&format!("{name} must be a whole number")))
}

#[allow(clippy::too_many_arguments)]
#[wasm_bindgen]
pub fn run(
    listing: &str,
    group_by: &str,
    sort_by: &str,
    order: &str,
    top_n: &str,
    units: &str,
    chart_width: &str,
    skip_folders: &str,
    ignore_case: &str,
    format: &str,
) -> Result<String, JsValue> {
    let opts = Options {
        group_by: text_or(group_by, "extension"),
        sort_by: text_or(sort_by, "size"),
        order: text_or(order, "desc"),
        top_n: parse_u32("top_n", top_n, 15)?,
        units: text_or(units, "binary"),
        chart_width: parse_u32("chart_width", chart_width, 32)?,
        skip_folders: boolish(skip_folders, true),
        ignore_case: boolish(ignore_case, true),
        format: text_or(format, "chart"),
    };
    gizza_ai_disk_usage_by_filetype_core::run(listing, &opts).map_err(|e| JsValue::from_str(&e))
}
