//! Browser-facing wasm-bindgen wrapper for /tools/json-to-cbor/.
//! Field order MUST match meta.toml: json, output, canonical, group.
use gizza_ai_json_to_cbor_core::{run_with_options, Options};
use wasm_bindgen::prelude::*;

fn truthy(s: &str) -> bool {
    matches!(
        s.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "on" | "yes"
    )
}

#[wasm_bindgen]
pub fn run(json: &str, output: &str, canonical: &str, group: f64) -> Result<String, JsValue> {
    let opts = Options {
        output: if output.trim().is_empty() {
            "hex".into()
        } else {
            output.into()
        },
        canonical: canonical.trim().is_empty() || truthy(canonical),
        group: if group.is_finite() && group >= 0.0 {
            group.round() as u32
        } else {
            0
        },
    };
    run_with_options(json, &opts).map_err(|e| JsValue::from_str(&e))
}
