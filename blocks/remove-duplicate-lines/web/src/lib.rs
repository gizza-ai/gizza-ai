//! Browser-facing wasm-bindgen wrapper for /tools/remove-duplicate-lines/.
//! Field order MUST match meta.toml: text, ignore_case, trim, adjacent_only,
//! keep, remove_empty. All fields arrive as strings.
use gizza_ai_remove_duplicate_lines_core::{render, Keep, Options};
use wasm_bindgen::prelude::*;

fn truthy(s: &str) -> bool {
    matches!(s.trim().to_ascii_lowercase().as_str(), "true" | "1" | "on" | "yes")
}

#[wasm_bindgen]
pub fn run(
    text: &str,
    ignore_case: &str,
    trim: &str,
    adjacent_only: &str,
    keep: &str,
    remove_empty: &str,
) -> Result<String, JsValue> {
    let keep = Keep::parse(keep).map_err(|e| JsValue::from_str(&e))?;
    let opts = Options {
        ignore_case: truthy(ignore_case),
        trim: truthy(trim),
        adjacent_only: truthy(adjacent_only),
        keep,
        remove_empty: truthy(remove_empty),
    };
    render(text, &opts).map_err(|e| JsValue::from_str(&e))
}
