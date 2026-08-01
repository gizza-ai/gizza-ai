//! Browser-facing wasm-bindgen wrapper for /tools/ics-merge-dedupe/.
//!
//! tool.js passes EVERY page field as a raw string (no coercion for pure tools),
//! so this export takes `&str` for every param and converts the checkbox
//! (boolean) field here; the core owns all validation. Param order MUST match
//! page/meta.toml's [[input]] order.
use gizza_ai_ics_merge_dedupe_core::merge_str;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(
    ics: &str,
    dedupe_by: &str,
    keep: &str,
    sort: &str,
    calendar_name: &str,
) -> Result<String, JsValue> {
    merge_str(ics, dedupe_by, keep, parse_bool(sort), calendar_name).map_err(|e| JsValue::from_str(&e))
}

fn parse_bool(s: &str) -> bool {
    matches!(
        s.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "on" | "yes"
    )
}
