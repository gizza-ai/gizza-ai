//! Browser-facing wasm-bindgen wrapper for /tools/openapi-to-typescript-types/.
//! The standalone page passes every field value as a string, so the boolean and
//! numeric params arrive as strings and are parsed here (blank → the schema
//! default).
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(
    spec: &str,
    input_format: &str,
    declaration: &str,
    enum_style: &str,
    optional_style: &str,
    export: &str,
    readonly: &str,
    sort: &str,
    indent: &str,
) -> Result<String, JsValue> {
    let indent = indent.trim().parse::<u32>().unwrap_or(2);
    gizza_ai_openapi_to_typescript_types_core::convert(
        spec,
        input_format,
        declaration,
        enum_style,
        optional_style,
        truthy(export, true),
        truthy(readonly, false),
        truthy(sort, false),
        indent,
    )
    .map_err(|e| JsValue::from_str(&e))
}

/// Parse a checkbox field value; blank falls back to `default`.
fn truthy(v: &str, default: bool) -> bool {
    match v.trim().to_ascii_lowercase().as_str() {
        "" => default,
        "true" | "1" | "on" | "yes" => true,
        _ => false,
    }
}
