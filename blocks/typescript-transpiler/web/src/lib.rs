//! Browser-facing wasm-bindgen wrapper for /tools/typescript-transpiler/.
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(input: &str, enum_style: &str, remove_comments: &str) -> Result<String, JsValue> {
    let enum_style = gizza_ai_typescript_transpiler_core::parse_enum_style(enum_style)
        .map_err(|e| JsValue::from_str(&e))?;
    let options = gizza_ai_typescript_transpiler_core::Options {
        enum_style,
        remove_comments: matches!(
            remove_comments.trim().to_ascii_lowercase().as_str(),
            "true" | "1" | "yes" | "on"
        ),
    };
    gizza_ai_typescript_transpiler_core::transpile(input, &options)
        .map_err(|e| JsValue::from_str(&e))
}
