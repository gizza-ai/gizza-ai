//! Browser-facing wasm-bindgen wrapper for /tools/http-header-normalizer/.
//! tool.js passes every page field as a raw string; this export takes `&str`
//! for each and parses the booleans here. Param order MUST match page/meta.toml.
use gizza_ai_http_header_normalizer_core::normalize;
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
    case: &str,
    sort: &str,
    duplicates: &str,
    unfold: &str,
    drop_empty: &str,
    drop_headers: &str,
    keep_headers: &str,
    output: &str,
) -> Result<String, JsValue> {
    normalize(
        input,
        case,
        sort,
        duplicates,
        flag(unfold),
        flag(drop_empty),
        drop_headers,
        keep_headers,
        output,
    )
    .map_err(|e| JsValue::from_str(&e))
}
