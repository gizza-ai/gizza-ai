//! Browser-facing wasm-bindgen wrapper for /tools/sbom-generator/.
//! Field order MUST match page/meta.toml.
use wasm_bindgen::prelude::*;

fn truthy_default_true(s: &str) -> bool {
    let t = s.trim().to_ascii_lowercase();
    if t.is_empty() {
        true
    } else {
        matches!(t.as_str(), "true" | "1" | "on" | "yes")
    }
}

#[allow(clippy::too_many_arguments)]
#[wasm_bindgen]
pub fn run(
    lockfile: &str,
    input_format: &str,
    output: &str,
    component_name: &str,
    component_version: &str,
    include_dev: &str,
    timestamp: &str,
    pretty: &str,
) -> Result<String, JsValue> {
    let input_format = if input_format.trim().is_empty() { "auto" } else { input_format };
    let output = if output.trim().is_empty() { "cyclonedx-json" } else { output };
    gizza_ai_sbom_generator_core::generate(
        lockfile,
        input_format,
        output,
        component_name,
        component_version,
        truthy_default_true(include_dev),
        timestamp,
        truthy_default_true(pretty),
    )
    .map_err(|e| JsValue::from_str(&e))
}
