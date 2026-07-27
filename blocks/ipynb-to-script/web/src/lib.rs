//! Browser-facing wasm-bindgen wrapper for /tools/ipynb-to-script/.
//! Field order MUST match meta.toml: notebook, output, include_markdown,
//! include_outputs, cell_markers.
use gizza_ai_ipynb_to_script_core::{convert, Output};
use wasm_bindgen::prelude::*;

fn truthy(s: &str) -> bool {
    matches!(s.trim().to_ascii_lowercase().as_str(), "true" | "1" | "on" | "yes")
}

#[wasm_bindgen]
pub fn run(
    notebook: &str,
    output: &str,
    include_markdown: &str,
    include_outputs: &str,
    cell_markers: &str,
) -> Result<String, JsValue> {
    let out = Output::parse(output).map_err(|e| JsValue::from_str(&e))?;
    convert(
        notebook,
        out,
        truthy(include_markdown),
        truthy(include_outputs),
        truthy(cell_markers),
    )
    .map_err(|e| JsValue::from_str(&e))
}
