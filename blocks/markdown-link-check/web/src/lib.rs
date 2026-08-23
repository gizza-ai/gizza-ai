//! Browser-facing wasm-bindgen wrapper for /tools/markdown-link-check/.
//! Field order MUST match meta.toml: markdown, link_kind, report_format, show_ok,
//! check_anchors, flag_insecure. Fields arrive as strings; booleans as "true"/"false".
use gizza_ai_markdown_link_check_core::run as check_run;
use wasm_bindgen::prelude::*;

fn truthy(v: &str) -> bool {
    matches!(v.trim(), "true" | "1" | "on" | "yes")
}

fn truthy_default_true(v: &str) -> bool {
    if v.trim().is_empty() {
        true
    } else {
        truthy(v)
    }
}

#[wasm_bindgen]
pub fn run(
    markdown: &str,
    link_kind: &str,
    report_format: &str,
    show_ok: &str,
    check_anchors: &str,
    flag_insecure: &str,
) -> Result<String, JsValue> {
    check_run(
        markdown,
        link_kind,
        report_format,
        truthy(show_ok),
        truthy_default_true(check_anchors),
        truthy(flag_insecure),
    )
    .map_err(|e| JsValue::from_str(&e))
}
