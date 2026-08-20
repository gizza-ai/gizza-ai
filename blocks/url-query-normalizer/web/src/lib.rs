//! Browser-facing wasm-bindgen wrapper for /tools/url-query-normalizer/.
//! tool.js passes every page field as a raw string; this export takes `&str`
//! for each and parses the booleans here. Param order MUST match page/meta.toml.
use gizza_ai_url_query_normalizer_core::normalize;
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
    input: &str,
    sort: &str,
    dedupe: &str,
    encoding: &str,
    space: &str,
    drop_tracking: &str,
    drop_params: &str,
    keep_params: &str,
    drop_empty: &str,
    output: &str,
) -> Result<String, JsValue> {
    normalize(
        input,
        sort,
        dedupe,
        encoding,
        space,
        flag(drop_tracking),
        drop_params,
        keep_params,
        flag(drop_empty),
        output,
    )
    .map_err(|e| JsValue::from_str(&e))
}
