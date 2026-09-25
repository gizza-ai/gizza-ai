//! Browser-facing wasm-bindgen wrapper for /tools/json-to-messagepack/.
//!
//! Field order MUST match page/meta.toml: input, output, key_order,
//! compact_floats, spec, group.
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(
    input: &str,
    output: &str,
    key_order: &str,
    compact_floats: &str,
    spec: &str,
    group: &str,
) -> Result<String, JsValue> {
    let opts = gizza_ai_json_to_messagepack_core::Options {
        output: defaulted(output, "hex"),
        key_order: defaulted(key_order, "input"),
        compact_floats: truthy(compact_floats),
        spec: defaulted(spec, "new"),
        group: parse_group(group).map_err(|e| JsValue::from_str(&e))?,
    };
    gizza_ai_json_to_messagepack_core::run_with_options(input, &opts)
        .map_err(|e| JsValue::from_str(&e))
}

fn defaulted(s: &str, default: &str) -> String {
    let t = s.trim();
    if t.is_empty() {
        default.to_string()
    } else {
        t.to_string()
    }
}

fn truthy(s: &str) -> bool {
    matches!(
        s.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "on" | "yes"
    )
}

fn parse_group(s: &str) -> Result<u32, String> {
    let t = s.trim();
    if t.is_empty() {
        return Ok(0);
    }
    let n: u32 = t
        .parse()
        .map_err(|_| format!("group must be an integer from 0 to 64 (got '{t}')"))?;
    if n > 64 {
        return Err("group must be at most 64".to_string());
    }
    Ok(n)
}
