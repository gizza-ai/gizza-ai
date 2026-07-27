//! Browser-facing wasm-bindgen wrapper for /tools/swagger2-to-openapi3/.
//! The standalone page passes every field value as a string, so `indent` and
//! `patch` arrive as strings and are parsed here (blank → the schema default).
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(
    spec: &str,
    input_format: &str,
    output_format: &str,
    target_version: &str,
    indent: &str,
    patch: &str,
) -> Result<String, JsValue> {
    let indent = indent.trim().parse::<u32>().unwrap_or(2);
    // A default-true checkbox sends "true"; unchecked sends "false".
    let patch = !matches!(patch.trim().to_ascii_lowercase().as_str(), "false" | "0" | "no" | "off");
    gizza_ai_swagger2_to_openapi3_core::convert(
        spec,
        input_format,
        output_format,
        target_version,
        indent,
        patch,
    )
    .map_err(|e| JsValue::from_str(&e))
}
