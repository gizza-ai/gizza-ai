//! Browser-facing wasm-bindgen wrapper for /tools/sbom-diff/.
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

#[wasm_bindgen]
pub fn run(
    old: &str,
    new: &str,
    old_format: &str,
    new_format: &str,
    include_dev: &str,
    output: &str,
) -> Result<String, JsValue> {
    let old_format = if old_format.trim().is_empty() { "auto" } else { old_format };
    let new_format = if new_format.trim().is_empty() { "auto" } else { new_format };
    let output = if output.trim().is_empty() { "text" } else { output };
    gizza_ai_sbom_diff_core::diff(
        old,
        new,
        old_format,
        new_format,
        truthy_default_true(include_dev),
        output,
    )
    .map_err(|e| JsValue::from_str(&e))
}
