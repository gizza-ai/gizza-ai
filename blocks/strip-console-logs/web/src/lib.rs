//! Browser-facing wasm-bindgen wrapper for /tools/strip-console-logs/.
//! Field order MUST match meta.toml: code, methods, keep, action, remove_debugger, output.
//! Fields arrive as strings; the checkbox arrives as "true"/"false".
use gizza_ai_strip_console_logs_core::strip;
use wasm_bindgen::prelude::*;

fn truthy(v: &str) -> bool {
    matches!(v.trim(), "true" | "1" | "on" | "yes")
}

#[wasm_bindgen]
pub fn run(
    code: &str,
    methods: &str,
    keep: &str,
    action: &str,
    remove_debugger: &str,
    output: &str,
) -> Result<String, JsValue> {
    strip(
        code,
        methods,
        keep,
        action,
        truthy(remove_debugger),
        output,
    )
    .map_err(|e| JsValue::from_str(&e))
}
