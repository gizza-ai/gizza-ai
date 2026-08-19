//! Browser-facing wasm-bindgen wrapper for /tools/lua-minifier/.
//! Field order MUST match page/meta.toml: code, remove_comments, keep_license,
//! rename_locals, line_breaks. Checkboxes arrive as "true"/"false" strings.
use wasm_bindgen::prelude::*;

fn truthy(s: &str) -> bool {
    matches!(
        s.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "on" | "yes"
    )
}

fn or_default<'a>(value: &'a str, fallback: &'a str) -> &'a str {
    if value.trim().is_empty() {
        fallback
    } else {
        value
    }
}

#[wasm_bindgen]
pub fn run(
    code: &str,
    remove_comments: &str,
    keep_license: &str,
    rename_locals: &str,
    line_breaks: &str,
) -> Result<String, JsValue> {
    gizza_ai_lua_minifier_core::run(
        code,
        truthy(remove_comments),
        truthy(keep_license),
        truthy(rename_locals),
        or_default(line_breaks, "strip"),
    )
    .map_err(|e| JsValue::from_str(&e))
}
