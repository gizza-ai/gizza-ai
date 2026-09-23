//! Browser-facing wasm-bindgen wrapper for /tools/pm-list-packages-parser/.
use wasm_bindgen::prelude::*;

/// Page checkboxes arrive as "true"/"false" strings; an empty value means the
/// field was never set, which for this default-on flag means "on".
fn truthy(s: &str) -> bool {
    !matches!(
        s.trim().to_ascii_lowercase().as_str(),
        "false" | "0" | "off" | "no"
    )
}

#[wasm_bindgen]
pub fn run(
    input: &str,
    filter: &str,
    format: &str,
    sort: &str,
    group: &str,
    disabled_list: &str,
    system_list: &str,
) -> Result<String, JsValue> {
    gizza_ai_pm_list_packages_parser_core::run(
        input,
        filter,
        format,
        sort,
        truthy(group),
        disabled_list,
        system_list,
    )
    .map_err(|e| JsValue::from_str(&e))
}
