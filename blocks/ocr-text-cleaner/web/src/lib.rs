//! Browser-facing wasm-bindgen wrapper for /tools/ocr-text-cleaner/.
//! The page passes every field as a raw string, so booleans are parsed here and
//! the whole request funnels through the shared `core::run` validation.
use wasm_bindgen::prelude::*;

fn truthy(v: &str, default: bool) -> bool {
    let s = v.trim().to_ascii_lowercase();
    if s.is_empty() {
        default
    } else {
        matches!(s.as_str(), "true" | "1" | "on" | "yes")
    }
}

#[allow(clippy::too_many_arguments)]
#[wasm_bindgen]
pub fn run(
    text: &str,
    fix_ligatures: &str,
    join_hyphenated: &str,
    line_breaks: &str,
    fix_confusables: &str,
    fix_rn: &str,
    fix_spacing: &str,
) -> Result<String, JsValue> {
    gizza_ai_ocr_text_cleaner_core::run(
        text,
        truthy(fix_ligatures, true),
        truthy(join_hyphenated, true),
        if line_breaks.trim().is_empty() {
            "keep"
        } else {
            line_breaks
        },
        truthy(fix_confusables, true),
        truthy(fix_rn, false),
        truthy(fix_spacing, true),
    )
    .map_err(|e| JsValue::from_str(&e))
}
