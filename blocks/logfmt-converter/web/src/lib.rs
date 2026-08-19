//! Browser-facing wasm-bindgen wrapper for /tools/logfmt-converter/.
//!
//! tool.js passes EVERY page field as a raw string (no coercion for pure tools),
//! so this export takes `&str` for every param and parses the bool fields here;
//! the core owns all validation (formats, delimiter, size cap). Param order MUST
//! match page/meta.toml's [[input]] order.
use gizza_ai_logfmt_converter_core::convert;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
#[allow(clippy::too_many_arguments)]
pub fn run(
    data: &str,
    from: &str,
    to: &str,
    delimiter: &str,
    detect_types: &str,
    pretty: &str,
    flatten: &str,
    keys: &str,
) -> Result<String, JsValue> {
    // detect_types + flatten default true (the descriptor renders both checkboxes
    // checked); pretty defaults false. Accept "true"/"1"/"on"/"yes" as truthy.
    let detect_types = parse_bool(detect_types, true);
    let pretty = parse_bool(pretty, false);
    let flatten = parse_bool(flatten, true);
    convert(
        data,
        from,
        to,
        delimiter,
        detect_types,
        pretty,
        flatten,
        keys,
    )
    .map_err(|e| JsValue::from_str(&e))
}

fn parse_bool(s: &str, default: bool) -> bool {
    match s.trim().to_ascii_lowercase().as_str() {
        "" => default,
        "true" | "1" | "on" | "yes" => true,
        _ => false,
    }
}
