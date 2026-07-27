//! Browser-facing wasm-bindgen wrapper for /tools/pem-bundle-splitter/.
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(pem: &str, output: &str, fingerprints: &str) -> Result<String, JsValue> {
    let mode = gizza_ai_pem_bundle_splitter_core::parse_output(output)
        .map_err(|e| JsValue::from_str(&e))?;
    let fingerprints = !matches!(
        fingerprints.trim().to_ascii_lowercase().as_str(),
        "false" | "0" | "off" | "no"
    );
    gizza_ai_pem_bundle_splitter_core::run(pem, mode, fingerprints)
        .map_err(|e| JsValue::from_str(&e))
}
