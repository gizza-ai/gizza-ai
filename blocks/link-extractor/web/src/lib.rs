//! Browser-facing wasm-bindgen wrapper for /tools/link-extractor/.
//! Field order MUST match meta.toml: input, source, base_url, dedup, output.
use gizza_ai_link_extractor_core::{render, Options, Output, Source};
use wasm_bindgen::prelude::*;

fn truthy(s: &str) -> bool {
    matches!(s.trim().to_ascii_lowercase().as_str(), "true" | "1" | "on" | "yes")
}

#[wasm_bindgen]
pub fn run(
    input: &str,
    source: &str,
    base_url: &str,
    dedup: &str,
    output: &str,
) -> Result<String, JsValue> {
    let src = Source::parse(source).map_err(|e| JsValue::from_str(&e))?;
    let out = Output::parse(output).map_err(|e| JsValue::from_str(&e))?;
    let base = base_url.trim();
    let opts = Options {
        base_url: if base.is_empty() { None } else { Some(base.to_string()) },
        dedup: truthy(dedup),
    };
    render(input, src, out, &opts).map_err(|e| JsValue::from_str(&e))
}
