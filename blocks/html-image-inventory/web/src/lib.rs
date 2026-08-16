//! Browser-facing wasm-bindgen wrapper for /tools/html-image-inventory/.
//! Field order MUST match meta.toml: html, format, include_sources,
//! only_issues, flag_empty_alt, include_summary.
use gizza_ai_html_image_inventory_core::{inventory, parse_format, Options};
use wasm_bindgen::prelude::*;

/// Page checkboxes arrive as "true"/"false" strings; treat anything positive as on.
fn truthy(s: &str) -> bool {
    matches!(
        s.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "on" | "yes"
    )
}

fn truthy_default_true(s: &str) -> bool {
    !matches!(
        s.trim().to_ascii_lowercase().as_str(),
        "false" | "0" | "off" | "no"
    )
}

#[wasm_bindgen]
pub fn run(
    html: &str,
    format: &str,
    include_sources: &str,
    only_issues: &str,
    flag_empty_alt: &str,
    include_summary: &str,
) -> Result<String, JsValue> {
    let fmt = parse_format(format).map_err(|e| JsValue::from_str(&e))?;
    inventory(
        html,
        fmt,
        &Options {
            include_sources: truthy_default_true(include_sources),
            only_issues: truthy(only_issues),
            flag_empty_alt: truthy(flag_empty_alt),
            include_summary: truthy_default_true(include_summary),
        },
    )
    .map_err(|e| JsValue::from_str(&e))
}
