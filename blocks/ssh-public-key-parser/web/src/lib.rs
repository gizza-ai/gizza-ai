//! Browser-facing wasm-bindgen wrapper for /tools/ssh-public-key-parser/.
//! Compiled with wasm-pack for the standalone page. The page passes every field as a
//! STRING, so the boolean toggles are parsed here; the wasm32-unknown-unknown target has
//! no std clock, so certificate validity is evaluated against the browser's own time
//! (`Date.now()`). Output is the pretty-printed JSON report.
use wasm_bindgen::prelude::*;

/// Page checkboxes arrive as "true"/"false"; be liberal about the accepted truthy forms.
fn truthy(v: &str) -> bool {
    matches!(
        v.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "on" | "yes"
    )
}

#[wasm_bindgen]
pub fn run(
    input: &str,
    expected_fingerprint: &str,
    include_sha1: &str,
    uppercase_md5: &str,
) -> Result<String, JsValue> {
    let opts = gizza_ai_ssh_public_key_parser_core::Options {
        include_sha1: truthy(include_sha1),
        uppercase_md5: truthy(uppercase_md5),
        expected_fingerprint: expected_fingerprint.to_string(),
        now: (js_sys::Date::now() / 1000.0) as i64,
    };
    gizza_ai_ssh_public_key_parser_core::run(input, &opts).map_err(|e| JsValue::from_str(&e))
}
