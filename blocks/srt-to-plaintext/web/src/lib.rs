//! Browser-facing wasm-bindgen wrapper for /tools/srt-to-plaintext/.
use wasm_bindgen::prelude::*;

fn truthy(v: &str) -> bool {
    matches!(
        v.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "yes" | "on"
    )
}

#[wasm_bindgen]
pub fn run(
    input: &str,
    layout: &str,
    strip_tags: &str,
    remove_sound_effects: &str,
    remove_speaker_labels: &str,
    dedupe: &str,
) -> Result<String, JsValue> {
    gizza_ai_srt_to_plaintext_core::convert(
        input,
        layout,
        truthy(strip_tags),
        truthy(remove_sound_effects),
        truthy(remove_speaker_labels),
        truthy(dedupe),
    )
    .map_err(|e| JsValue::from_str(&e))
}
