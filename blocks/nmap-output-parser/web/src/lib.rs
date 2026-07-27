//! Browser-facing wasm-bindgen wrapper for /tools/nmap-output-parser/.
//! Field order must match page/meta.toml.
use gizza_ai_nmap_output_parser_core::{parse, InputFormat, Options, OutputFormat, SortBy};
use wasm_bindgen::prelude::*;

fn truthy(v: &str) -> bool {
    matches!(
        v.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "on" | "yes"
    )
}

#[wasm_bindgen]
pub fn run(
    input: &str,
    format: &str,
    output: &str,
    sort_by: &str,
    open_only: &str,
) -> Result<String, JsValue> {
    let opts = Options {
        input_format: InputFormat::parse(format),
        output_format: OutputFormat::parse(output),
        sort_by: SortBy::parse(sort_by),
        open_only: truthy(open_only),
    };
    parse(input, opts).map_err(|e| JsValue::from_str(&e))
}
