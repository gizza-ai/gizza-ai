//! Browser-facing wasm-bindgen wrapper for /tools/svg-optimize/.
//! Field order MUST match meta.toml: svg, remove_comments, remove_metadata,
//! remove_ids, remove_dimensions. All fields arrive as strings.
use gizza_ai_svg_optimize_core::{optimize, Options};
use wasm_bindgen::prelude::*;

fn truthy(v: &str) -> bool {
    matches!(
        v.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "on" | "yes"
    )
}

#[wasm_bindgen]
pub fn run(
    svg: &str,
    remove_comments: &str,
    remove_metadata: &str,
    remove_ids: &str,
    remove_dimensions: &str,
) -> Result<String, JsValue> {
    let opts = Options {
        remove_comments: truthy(remove_comments),
        remove_metadata: truthy(remove_metadata),
        remove_ids: truthy(remove_ids),
        remove_dimensions: truthy(remove_dimensions),
    };
    optimize(svg, &opts).map_err(|e| JsValue::from_str(&e))
}
