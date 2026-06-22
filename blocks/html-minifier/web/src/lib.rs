//! Browser-facing wasm-bindgen wrapper for /tools/html-minifier/.
//! Field order MUST match meta.toml: html, remove_comments. Fields are strings.
use gizza_ai_html_minifier_core::minify;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(html: &str, remove_comments: &str) -> Result<String, JsValue> {
    // On the page the checkbox is opt-in: ticked = remove comments; unticked
    // (empty/false) = keep them. (The chat tool defaults to remove=true.)
    let rc = matches!(
        remove_comments.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "on" | "yes"
    );
    minify(html, rc).map_err(|e| JsValue::from_str(&e))
}
