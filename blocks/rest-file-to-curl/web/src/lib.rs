//! Browser-facing wasm-bindgen wrapper for /tools/rest-file-to-curl/.
//!
//! The page passes every field value as a string, in the meta.toml `[[input]]`
//! order. Booleans arrive as "true"/"false". The page target
//! (wasm32-unknown-unknown) has no std clock, so the `{{$timestamp}}` /
//! `{{$datetime}}` / `{{$guid}}` system variables are resolved from the browser
//! clock and passed into the core as an explicit timestamp.
use wasm_bindgen::prelude::*;

fn truthy(v: &str) -> bool {
    matches!(
        v.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "yes" | "on"
    )
}

/// Expand a `.http` / `.rest` / `.ain` request file into curl command text.
///
/// Throws a JS error string on any invalid input (unparseable request line,
/// unknown shell/format/flag style, out-of-range request selector, bad env).
#[allow(clippy::too_many_arguments)]
#[wasm_bindgen]
pub fn run(
    data: &str,
    env: &str,
    environment: &str,
    request: &str,
    format: &str,
    shell: &str,
    flag_style: &str,
    multiline: &str,
    unresolved: &str,
    follow_redirects: &str,
    compressed: &str,
    insecure: &str,
    include_comments: &str,
) -> Result<String, JsValue> {
    gizza_ai_rest_file_to_curl_core::convert(
        data,
        env,
        environment,
        request,
        format,
        shell,
        flag_style,
        truthy(multiline),
        unresolved,
        truthy(follow_redirects),
        truthy(compressed),
        truthy(insecure),
        truthy(include_comments),
        js_sys::Date::now() as u64,
    )
    .map_err(|e| JsValue::from_str(&e))
}
