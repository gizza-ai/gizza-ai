//! Browser-facing wasm-bindgen wrapper for /tools/find-replace/.
//! Field order MUST match meta.toml: text, find, replace, regex, case_sensitive,
//! global. Booleans arrive as strings from the page form.
use gizza_ai_find_replace_core::find_replace;
use wasm_bindgen::prelude::*;

fn truthy(s: &str) -> bool {
    matches!(s.trim().to_ascii_lowercase().as_str(), "true" | "1" | "on" | "yes")
}
fn truthy_default_true(s: &str) -> bool {
    !matches!(s.trim().to_ascii_lowercase().as_str(), "false" | "0" | "off" | "no")
}

#[wasm_bindgen]
pub fn run(
    text: &str,
    find: &str,
    replace: &str,
    regex: &str,
    case_sensitive: &str,
    global: &str,
) -> Result<String, JsValue> {
    let (out, _count) = find_replace(
        text,
        find,
        replace,
        truthy(regex),
        truthy_default_true(case_sensitive),
        truthy_default_true(global),
    )
    .map_err(|e| JsValue::from_str(&e))?;
    Ok(out)
}
