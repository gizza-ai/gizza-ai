//! Browser-facing wasm-bindgen wrapper for /tools/iam-policy-linter/.
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(
    policy: &str,
    policy_type: &str,
    format: &str,
    min_severity: &str,
    ignore: &str,
) -> Result<String, JsValue> {
    gizza_ai_iam_policy_linter_core::render(policy, policy_type, format, min_severity, ignore)
        .map_err(|e| JsValue::from_str(&e))
}
