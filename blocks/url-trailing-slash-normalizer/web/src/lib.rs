//! Browser-facing wasm-bindgen wrapper for /tools/url-trailing-slash-normalizer/.
//! tool.js passes every page field as a raw string; this export takes `&str`
//! for each and parses the booleans here. Param order MUST match page/meta.toml.
use gizza_ai_url_trailing_slash_normalizer_core::normalize;
use wasm_bindgen::prelude::*;

fn flag(v: &str) -> bool {
    matches!(
        v.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "on" | "yes"
    )
}

#[wasm_bindgen]
pub fn run(
    urls: &str,
    mode: &str,
    skip_file_paths: &str,
    normalize_root: &str,
    dedupe: &str,
    on_invalid: &str,
    output: &str,
) -> Result<String, JsValue> {
    normalize(
        urls,
        mode,
        flag(skip_file_paths),
        flag(normalize_root),
        flag(dedupe),
        on_invalid,
        output,
    )
    .map_err(|e| JsValue::from_str(&e))
}
