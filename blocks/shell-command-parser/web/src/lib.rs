//! Browser-facing wasm-bindgen wrapper for /tools/shell-command-parser/.
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(input: &str, format: Option<String>, pretty: Option<bool>) -> Result<String, JsValue> {
    let format = format.as_deref().unwrap_or("json");
    let pretty = pretty.unwrap_or(true);
    gizza_ai_shell_command_parser_core::run(input, format, pretty)
        .map_err(|e| JsValue::from_str(&e))
}
