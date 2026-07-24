//! Browser-facing wasm-bindgen wrapper for /tools/cyberchef-pipeline/.
//! Field order MUST match meta.toml: input, recipe, output_format. All fields
//! arrive as strings (the output_format <select> sends its canonical value).
use gizza_ai_cyberchef_pipeline_core::{run as run_recipe, OutputFormat, Options};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(input: &str, recipe: &str, output_format: &str) -> Result<String, JsValue> {
    let opts = Options {
        output_format: OutputFormat::parse(output_format).map_err(|e| JsValue::from_str(&e))?,
    };
    run_recipe(input, recipe, &opts).map_err(|e| JsValue::from_str(&e))
}
