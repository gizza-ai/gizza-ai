//! Browser-facing wasm-bindgen wrapper for /tools/path-extractor/.
//! Field order MUST match page/meta.toml. Every field arrives as a string, so
//! checkboxes are parsed positive-truthy ("true"/"1"/"on"/"yes").
use gizza_ai_path_extractor_core::run;
use wasm_bindgen::prelude::*;

fn flag(v: &str) -> bool {
    matches!(
        v.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "on" | "yes"
    )
}

#[wasm_bindgen]
#[allow(clippy::too_many_arguments)]
pub fn run_extract(
    text: &str,
    path_style: &str,
    require_separator: &str,
    keep_line_numbers: &str,
    output: &str,
    extensions: &str,
    extension_mode: &str,
    dedupe: &str,
    sort: &str,
    format: &str,
) -> Result<String, JsValue> {
    run(
        text,
        path_style,
        flag(require_separator),
        flag(keep_line_numbers),
        output,
        extensions,
        extension_mode,
        flag(dedupe),
        sort,
        format,
    )
    .map_err(|e| JsValue::from_str(&e))
}
