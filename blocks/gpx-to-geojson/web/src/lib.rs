//! Browser-facing wasm-bindgen wrapper for /tools/gpx-to-geojson/.
//! Field order MUST match meta.toml: input, output_format, include_styles.
//! Fields arrive as strings (checkboxes send "true"/"false").
use gizza_ai_gpx_to_geojson_core::{convert, OutputFormat, Options};
use wasm_bindgen::prelude::*;

fn truthy(s: &str) -> bool {
    matches!(s.trim().to_ascii_lowercase().as_str(), "true" | "1" | "on" | "yes")
}

#[wasm_bindgen]
pub fn run(input: &str, output_format: &str, include_styles: &str) -> Result<String, JsValue> {
    let fmt = if output_format.is_empty() { "geojson" } else { output_format };
    let output_format = OutputFormat::parse(fmt).map_err(|e| JsValue::from_str(&e))?;
    let opt = Options { output_format, include_styles: truthy(include_styles) };
    convert(input, &opt).map_err(|e| JsValue::from_str(&e))
}
