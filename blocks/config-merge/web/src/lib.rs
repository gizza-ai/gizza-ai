//! Browser-facing wasm-bindgen wrapper for /tools/config-merge/.
//! Field order MUST match meta.toml: layer1, layer2, layer3, layer4,
//! layer_names, input_format, output, object_merge, array_merge, key_case, null_deletes,
//! substitute, vars, sort_keys, indent.
use gizza_ai_config_merge_core::{merge_config, Request};
use wasm_bindgen::prelude::*;

fn truthy(v: &str) -> bool {
    matches!(
        v.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "on" | "yes"
    )
}

#[wasm_bindgen]
#[allow(clippy::too_many_arguments)]
pub fn run(
    layer1: &str,
    layer2: &str,
    layer3: &str,
    layer4: &str,
    layer_names: &str,
    input_format: &str,
    output: &str,
    object_merge: &str,
    array_merge: &str,
    key_case: &str,
    null_deletes: &str,
    substitute: &str,
    vars: &str,
    sort_keys: &str,
    indent: &str,
) -> Result<String, JsValue> {
    let indent: usize = match indent.trim() {
        "" => 2,
        raw => raw.parse().map_err(|_| {
            JsValue::from_str(&format!("indent must be a whole number 1-8, got '{raw}'"))
        })?,
    };
    merge_config(Request {
        layers: [layer1, layer2, layer3, layer4],
        layer_names,
        input_format,
        output,
        object_merge,
        array_merge,
        key_case,
        null_deletes: truthy(null_deletes),
        substitute: truthy(substitute),
        vars,
        sort_keys: truthy(sort_keys),
        indent,
    })
    .map_err(|e| JsValue::from_str(&e))
}
