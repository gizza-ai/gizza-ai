//! Browser-facing wasm-bindgen wrapper for /tools/cloud-share-direct-link/.
//! tool.js passes every page field as a raw string; this export takes `&str`
//! for each and parses the bool here. Param order MUST match page/meta.toml.
use gizza_ai_cloud_share_direct_link_core::convert;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
#[allow(clippy::too_many_arguments)]
pub fn run(
    url: &str,
    provider: &str,
    mode: &str,
    output: &str,
    onedrive_style: &str,
    docs_export: &str,
    file: &str,
    per_line: &str,
) -> Result<String, JsValue> {
    let per_line = matches!(
        per_line.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "on" | "yes"
    );
    convert(
        url,
        provider,
        mode,
        output,
        onedrive_style,
        docs_export,
        file,
        per_line,
    )
    .map_err(|e| JsValue::from_str(&e))
}
