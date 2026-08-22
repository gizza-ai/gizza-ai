//! Browser-facing wasm-bindgen wrapper for /tools/rust-module-map/.
use gizza_ai_rust_module_map_core::{module_map, Options};
use wasm_bindgen::prelude::*;

fn truthy(v: &str) -> bool {
    matches!(
        v.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "on" | "yes"
    )
}

fn or_default<'a>(v: &'a str, fallback: &'a str) -> &'a str {
    if v.trim().is_empty() {
        fallback
    } else {
        v
    }
}

fn parse_depth(raw: &str) -> Result<u32, JsValue> {
    let t = raw.trim();
    if t.is_empty() {
        return Ok(0);
    }
    let n: u32 = t
        .parse()
        .map_err(|_| JsValue::from_str(&format!("max_depth must be a whole number (got {t:?})")))?;
    if n > 64 {
        return Err(JsValue::from_str("max_depth must be between 0 and 64"));
    }
    Ok(n)
}

#[allow(clippy::too_many_arguments)]
#[wasm_bindgen]
pub fn run(
    source: &str,
    format: &str,
    max_depth: &str,
    focus_on: &str,
    sort_by: &str,
    show_types: &str,
    show_traits: &str,
    show_fns: &str,
    show_impls: &str,
    show_consts: &str,
    include_tests: &str,
    show_visibility: &str,
    crate_name: &str,
) -> Result<String, JsValue> {
    let opts = Options {
        format: or_default(format, "tree").to_string(),
        max_depth: parse_depth(max_depth)?,
        focus_on: focus_on.to_string(),
        sort_by: or_default(sort_by, "source").to_string(),
        show_types: truthy(show_types),
        show_traits: truthy(show_traits),
        show_fns: truthy(show_fns),
        show_impls: truthy(show_impls),
        show_consts: truthy(show_consts),
        include_tests: truthy(include_tests),
        show_visibility: truthy(show_visibility),
        crate_name: crate_name.to_string(),
    };
    module_map(source, &opts).map_err(|e| JsValue::from_str(&e))
}
