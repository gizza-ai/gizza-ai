//! Browser-facing wasm-bindgen wrapper for /tools/tcx-to-gpx/.
//! Field order MUST match meta.toml: input, include_extensions.
//! Fields arrive as strings (checkboxes send "true"/"false").
use gizza_ai_tcx_to_gpx_core::{convert, Options};
use wasm_bindgen::prelude::*;

fn truthy(s: &str) -> bool {
    matches!(s.trim().to_ascii_lowercase().as_str(), "true" | "1" | "on" | "yes")
}

#[wasm_bindgen]
pub fn run(input: &str, include_extensions: &str) -> Result<String, JsValue> {
    let opt = Options { include_extensions: truthy(include_extensions) };
    convert(input, &opt).map_err(|e| JsValue::from_str(&e))
}
