//! Browser-facing wasm-bindgen wrapper for /tools/zero-width-cleaner/.
//! Field order MUST match meta.toml: text, remove_zero_width, remove_bidi,
//! remove_soft_hyphen, replace_nbsp, replacement.
use gizza_ai_zero_width_cleaner_core::clean;
use wasm_bindgen::prelude::*;

fn truthy(v: &str) -> bool {
    matches!(
        v.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "on" | "yes"
    )
}

#[wasm_bindgen]
pub fn run(
    text: &str,
    remove_zero_width: &str,
    remove_bidi: &str,
    remove_soft_hyphen: &str,
    replace_nbsp: &str,
    replacement: &str,
) -> Result<String, JsValue> {
    Ok(clean(
        text,
        truthy(remove_zero_width),
        truthy(remove_bidi),
        truthy(remove_soft_hyphen),
        truthy(replace_nbsp),
        replacement,
    ))
}
