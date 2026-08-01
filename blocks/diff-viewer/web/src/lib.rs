//! Browser-facing wasm-bindgen wrapper for /tools/diff-viewer/.
//! Compiled with wasm-pack for the standalone /tools/diff-viewer/ page.
use wasm_bindgen::prelude::*;

/// Render a pasted unified `diff` in the chosen `view`.
///
/// The tool page passes every field value as a string, so `ignore_whitespace`
/// arrives as a string and is coerced here. `view` is one of `inline`
/// (blank → this), `side-by-side`, `stats`, `json`.
///
/// Throws a JS error string when the input contains no recognizable diff or
/// `view` is unknown.
#[wasm_bindgen]
pub fn run(diff: &str, view: &str, ignore_whitespace: &str) -> Result<String, JsValue> {
    let view = if view.trim().is_empty() { "inline" } else { view };
    let ignore_whitespace = truthy(ignore_whitespace);
    gizza_ai_diff_viewer_core::run(diff, view, ignore_whitespace).map_err(|e| JsValue::from_str(&e))
}

fn truthy(s: &str) -> bool {
    matches!(
        s.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "yes" | "on"
    )
}
