//! Browser-facing wasm-bindgen wrapper for /tools/html-form-field-extractor/.
//! Field order MUST match meta.toml: html, format, form_index, include_buttons,
//! include_hidden, include_labels.
use gizza_ai_html_form_field_extractor_core::{extract, parse_format, Options};
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
    form_index: &str,
    include_buttons: &str,
    include_hidden: &str,
    include_labels: &str,
) -> Result<String, JsValue> {
    let fmt = parse_format(format).map_err(|e| JsValue::from_str(&e))?;
    let idx: i64 = form_index.trim().parse().unwrap_or(-1);
    extract(
        html,
        fmt,
        &Options {
            form_index: idx,
            include_buttons: truthy(include_buttons),
            include_hidden: truthy_default_true(include_hidden),
            include_labels: truthy_default_true(include_labels),
        },
    )
    .map_err(|e| JsValue::from_str(&e))
}
