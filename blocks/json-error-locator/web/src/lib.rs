//! Browser-facing wasm-bindgen wrapper for /tools/json-error-locator/.
//! Field order MUST match page/meta.toml: json, output, context_lines,
//! scan_all. The page driver passes every field as a raw string, so the number
//! and boolean are parsed here and all validation stays in the shared core.
use gizza_ai_json_error_locator_core::locate;
use wasm_bindgen::prelude::*;

fn parse_bool(s: &str, default: bool) -> bool {
    match s.trim().to_ascii_lowercase().as_str() {
        "" => default,
        "true" | "1" | "yes" | "on" => true,
        "false" | "0" | "no" | "off" => false,
        _ => default,
    }
}

#[wasm_bindgen]
pub fn run(
    json: &str,
    output: &str,
    context_lines: &str,
    scan_all: &str,
) -> Result<String, JsValue> {
    let out = match output.trim() {
        "" => "report",
        o => o,
    };
    let ctx = match context_lines.trim() {
        "" => 2usize,
        n => n.parse::<usize>().map_err(|_| {
            JsValue::from_str(&format!("context_lines '{n}' is not a whole number 0-10"))
        })?,
    };
    locate(json, out, ctx, parse_bool(scan_all, true)).map_err(|e| JsValue::from_str(&e))
}
