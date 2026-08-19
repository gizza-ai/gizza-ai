//! Browser-facing wasm-bindgen wrapper for /tools/ffmpeg-filtergraph-builder/.
//! Field order MUST match meta.toml: steps, stream, output, input_label,
//! output_label, input_file, output_file, explain.
use gizza_ai_ffmpeg_filtergraph_builder_core::build_from_strs;
use wasm_bindgen::prelude::*;

/// `explain` arrives from the page as "true"/"false" (checkbox) — parse positive-truthy.
#[wasm_bindgen]
#[allow(clippy::too_many_arguments)]
pub fn run(
    steps: &str,
    stream: &str,
    output: &str,
    input_label: &str,
    output_label: &str,
    input_file: &str,
    output_file: &str,
    explain: &str,
) -> Result<String, JsValue> {
    let explain = matches!(
        explain.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "on" | "yes"
    );
    build_from_strs(
        steps,
        stream,
        output,
        input_label,
        output_label,
        input_file,
        output_file,
        explain,
    )
    .map_err(|e| JsValue::from_str(&e))
}
