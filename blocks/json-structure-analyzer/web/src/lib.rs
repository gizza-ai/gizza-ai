//! Browser-facing wasm-bindgen wrapper for /tools/json-structure-analyzer/.
//! Field order MUST match meta.toml: json, format, top_keys, top_paths.
use gizza_ai_json_structure_analyzer_core::{analyze, Format, Options};
use wasm_bindgen::prelude::*;

fn format_from(s: &str) -> Format {
    match s.trim().to_ascii_lowercase().as_str() {
        "text" | "txt" | "plain" => Format::Text,
        _ => Format::Json,
    }
}

fn parse_cap(s: &str, default: usize) -> usize {
    match s.trim() {
        "" => default,
        t => t.parse::<usize>().unwrap_or(default),
    }
}

#[wasm_bindgen]
pub fn run(json: &str, format: &str, top_keys: &str, top_paths: &str) -> Result<String, JsValue> {
    let opts = Options {
        format: format_from(format),
        top_keys: parse_cap(top_keys, 30),
        top_paths: parse_cap(top_paths, 50),
    };
    analyze(json, &opts).map_err(|e| JsValue::from_str(&e))
}
