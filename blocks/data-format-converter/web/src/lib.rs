//! Browser-facing wasm-bindgen wrapper for /tools/data-format-converter/.
//!
//! tool.js passes EVERY page field as a raw string (no coercion for pure tools),
//! so this export takes `&str` for every param and parses the enum/bool fields
//! here; the core owns all validation. Param order MUST match page/meta.toml's
//! [[input]] order.
use gizza_ai_data_format_converter_core::{convert, Format};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(
    data: &str,
    from: &str,
    to: &str,
    headers: &str,
    infer_types: &str,
    pretty: &str,
    flatten: &str,
) -> Result<String, JsValue> {
    let from = Format::parse(from).map_err(|e| JsValue::from_str(&e))?;
    let to = Format::parse(to).map_err(|e| JsValue::from_str(&e))?;
    // headers + infer_types default true (the common CSV->JSON case); pretty and
    // flatten default false. Accept "true"/"1"/"on"/"yes" as truthy.
    let headers = parse_bool(headers, true);
    let infer_types = parse_bool(infer_types, true);
    let pretty = parse_bool(pretty, false);
    let flatten = parse_bool(flatten, false);
    convert(data, from, to, headers, infer_types, pretty, flatten).map_err(|e| JsValue::from_str(&e))
}

fn parse_bool(s: &str, default: bool) -> bool {
    match s.trim().to_ascii_lowercase().as_str() {
        "" => default,
        "true" | "1" | "on" | "yes" => true,
        _ => false,
    }
}
