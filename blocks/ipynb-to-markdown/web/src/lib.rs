//! Browser-facing wasm-bindgen wrapper for /tools/ipynb-to-markdown/.
//! Field order MUST match meta.toml: notebook, include_code, include_outputs,
//! include_markdown, show_prompts, image_mode.
use gizza_ai_ipynb_to_markdown_core::{convert, ImageMode, Options};
use wasm_bindgen::prelude::*;

fn truthy(s: &str) -> bool {
    matches!(s.trim().to_ascii_lowercase().as_str(), "true" | "1" | "on" | "yes")
}

#[wasm_bindgen]
pub fn run(
    notebook: &str,
    include_code: &str,
    include_outputs: &str,
    include_markdown: &str,
    show_prompts: &str,
    image_mode: &str,
) -> Result<String, JsValue> {
    let opts = Options {
        include_code: truthy(include_code),
        include_outputs: truthy(include_outputs),
        include_markdown: truthy(include_markdown),
        show_prompts: truthy(show_prompts),
        image_mode: ImageMode::parse(image_mode).map_err(|e| JsValue::from_str(&e))?,
    };
    convert(notebook, opts).map_err(|e| JsValue::from_str(&e))
}
