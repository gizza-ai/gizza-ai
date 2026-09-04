//! Browser-facing wasm-bindgen wrapper for /tools/gltf-glb-converter/.
//! Compiled with wasm-pack for the standalone page.
//!
//! Field order MUST match page/meta.toml: model, bin, input_format, to, output,
//! images, buffer_uri, pretty, output_encoding. The page passes every field as a
//! string, so blanks fall back to the descriptor defaults inside the core.
use wasm_bindgen::prelude::*;

/// The page sends checkboxes as `"true"`/`"false"`; be liberal about the rest.
fn truthy(value: &str) -> bool {
    matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "on" | "yes"
    )
}

/// Convert a glTF 2.0 asset between the .gltf and .glb containers.
///
/// - `model`: glTF JSON text, or GLB bytes as base64 / hex / a `data:` URL.
/// - `bin`: optional external buffer bytes (base64 / hex / `data:` URL).
/// - `input_format`: `"auto"` (default), `"gltf"`, `"base64"` or `"hex"`.
/// - `to`: `"auto"` (default, flip), `"glb"` or `"gltf"`.
/// - `output`: `"file"` (default), `"summary"` or `"buffer"`.
/// - `images`: `"auto"` (default), `"buffer"` or `"uri"`.
/// - `buffer_uri`: external buffer uri for glTF output (blank embeds a data: URI).
/// - `pretty`: `"true"` (default) pretty-prints the glTF JSON.
/// - `output_encoding`: `"data-url"` (default), `"base64"` or `"hex"`.
///
/// Throws a JS error string on invalid arguments or an unparseable model.
#[allow(clippy::too_many_arguments)]
#[wasm_bindgen]
pub fn run(
    model: &str,
    bin: &str,
    input_format: &str,
    to: &str,
    output: &str,
    images: &str,
    buffer_uri: &str,
    pretty: &str,
    output_encoding: &str,
) -> Result<String, JsValue> {
    gizza_ai_gltf_glb_converter_core::run(
        model,
        bin,
        input_format,
        to,
        output,
        images,
        buffer_uri,
        truthy(pretty),
        output_encoding,
    )
    .map_err(|e| JsValue::from_str(&e))
}
