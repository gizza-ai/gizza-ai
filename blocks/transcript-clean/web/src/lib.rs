//! Browser-facing wasm-bindgen wrapper for /tools/transcript-clean/.
use wasm_bindgen::prelude::*;

fn truthy(s: &str) -> bool {
    !matches!(s.trim().to_ascii_lowercase().as_str(), "false" | "0" | "off" | "no")
}

#[wasm_bindgen]
#[allow(clippy::too_many_arguments)]
pub fn run(
    input: &str,
    filler_level: &str,
    extra_fillers: &str,
    remove_timestamps: &str,
    remove_brackets: &str,
    merge_speakers: &str,
    fix_capitalization: &str,
    fix_punctuation: &str,
) -> Result<String, JsValue> {
    let filler_level = if filler_level.trim().is_empty() { "standard" } else { filler_level };
    gizza_ai_transcript_clean_core::clean(
        input,
        filler_level,
        extra_fillers,
        truthy(remove_timestamps),
        truthy(remove_brackets),
        truthy(merge_speakers),
        truthy(fix_capitalization),
        truthy(fix_punctuation),
    )
    .map_err(|e| JsValue::from_str(&e))
}
