//! Browser-facing wasm-bindgen wrapper for /tools/merge-conflict-resolver/.
use wasm_bindgen::prelude::*;

fn truthy_default_off(v: &str) -> bool {
    matches!(
        v.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "on" | "yes"
    )
}

#[wasm_bindgen]
pub fn run(
    text: &str,
    strategy: &str,
    choices: &str,
    output: &str,
    strict: &str,
) -> Result<String, JsValue> {
    gizza_ai_merge_conflict_resolver_core::resolve(
        text,
        strategy,
        choices,
        output,
        truthy_default_off(strict),
    )
    .map_err(|e| JsValue::from_str(&e))
}
