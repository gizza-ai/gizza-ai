//! Browser-facing wasm-bindgen wrapper for /tools/color-code-extractor/.
//! Field order MUST match page/meta.toml: text, output_format, color_format, sort,
//! include_counts, include_named, exclude_grey, exclude_monochrome, uppercase,
//! limit, var_prefix.
use wasm_bindgen::prelude::*;

fn truthy(s: &str) -> bool {
    matches!(
        s.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "on" | "yes"
    )
}

#[allow(clippy::too_many_arguments)]
#[wasm_bindgen]
pub fn run(
    text: &str,
    output_format: &str,
    color_format: &str,
    sort: &str,
    include_counts: &str,
    include_named: &str,
    exclude_grey: &str,
    exclude_monochrome: &str,
    uppercase: &str,
    limit: &str,
    var_prefix: &str,
) -> Result<String, JsValue> {
    let n: i64 = if limit.trim().is_empty() {
        0
    } else {
        limit
            .trim()
            .parse()
            .map_err(|_| JsValue::from_str("limit must be a whole number between 0 and 1000"))?
    };
    gizza_ai_color_code_extractor_core::extract(
        text,
        output_format,
        color_format,
        sort,
        truthy(include_counts),
        truthy(include_named),
        truthy(exclude_grey),
        truthy(exclude_monochrome),
        truthy(uppercase),
        n,
        var_prefix,
    )
    .map_err(|e| JsValue::from_str(&e))
}
