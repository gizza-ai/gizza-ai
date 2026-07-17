//! Browser-facing wasm-bindgen wrapper for /tools/env-to-configmap/.
//! Field order MUST match meta.toml: env, kind, name, namespace, secret_encoding, labels.
use gizza_ai_env_to_configmap_core::to_manifest;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(
    env: &str,
    kind: &str,
    name: &str,
    namespace: &str,
    secret_encoding: &str,
    labels: &str,
) -> Result<String, JsValue> {
    to_manifest(env, kind, name, namespace, secret_encoding, labels).map_err(|e| JsValue::from_str(&e))
}
