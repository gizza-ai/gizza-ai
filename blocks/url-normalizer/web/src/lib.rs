//! Browser-facing wasm-bindgen wrapper for /tools/url-normalizer/.
//! tool.js passes every page field as a raw string; this export takes `&str`
//! for each and parses the booleans here. Param order MUST match page/meta.toml.
use gizza_ai_url_normalizer_core::run as normalize;
use wasm_bindgen::prelude::*;

fn flag(v: &str) -> bool {
    matches!(
        v.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "on" | "yes"
    )
}

#[wasm_bindgen]
#[allow(clippy::too_many_arguments)]
pub fn run(
    urls: &str,
    base: &str,
    scheme: &str,
    www: &str,
    strip_default_port: &str,
    dot_segments: &str,
    collapse_slashes: &str,
    lowercase_path: &str,
    encoding: &str,
    drop_index: &str,
    trailing_slash: &str,
    sort_query: &str,
    dedupe_query: &str,
    drop_empty_params: &str,
    drop_tracking: &str,
    drop_fragment: &str,
    dedupe_urls: &str,
    on_invalid: &str,
    output: &str,
) -> Result<String, JsValue> {
    normalize(
        urls,
        base,
        scheme,
        www,
        flag(strip_default_port),
        flag(dot_segments),
        flag(collapse_slashes),
        flag(lowercase_path),
        encoding,
        flag(drop_index),
        trailing_slash,
        sort_query,
        flag(dedupe_query),
        flag(drop_empty_params),
        flag(drop_tracking),
        flag(drop_fragment),
        flag(dedupe_urls),
        on_invalid,
        output,
    )
    .map_err(|e| JsValue::from_str(&e))
}
