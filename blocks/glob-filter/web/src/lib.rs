//! Browser-facing wasm-bindgen wrapper for /tools/glob-filter/.
//! Field order MUST match meta.toml: paths, include, exclude, syntax,
//! case_sensitive, output. Every field arrives as a string (checkboxes send
//! "true"/"false", <select> sends the option value).
use gizza_ai_glob_filter_core::{render, OutputMode, Syntax};
use wasm_bindgen::prelude::*;

fn truthy(s: &str) -> bool {
    matches!(
        s.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "on" | "yes"
    )
}

#[wasm_bindgen]
pub fn run(
    paths: &str,
    include: &str,
    exclude: &str,
    syntax: &str,
    case_sensitive: &str,
    output: &str,
) -> Result<String, JsValue> {
    let syn = Syntax::parse(syntax).map_err(|e| JsValue::from_str(&e))?;
    let out = OutputMode::parse(output).map_err(|e| JsValue::from_str(&e))?;
    render(paths, include, exclude, syn, truthy(case_sensitive), out)
        .map_err(|e| JsValue::from_str(&e))
}
