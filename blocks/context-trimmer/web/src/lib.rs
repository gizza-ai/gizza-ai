//! Browser-facing wasm-bindgen wrapper for /tools/context-trimmer/.
//! Compiled with wasm-pack for the standalone /tools/context-trimmer/ page.
use gizza_ai_context_trimmer_core::Keep;
use wasm_bindgen::prelude::*;

fn truthy(v: &str) -> bool {
    matches!(v.trim().to_ascii_lowercase().as_str(), "true" | "1" | "yes" | "on")
}

#[wasm_bindgen]
pub fn run(
    text: &str,
    max_tokens: &str,
    chars_per_token: &str,
    keep: &str,
    marker: &str,
    head_ratio: &str,
    break_words: &str,
) -> Result<String, JsValue> {
    let max_tokens = max_tokens.trim().parse::<u32>().unwrap_or(512);
    let chars_per_token = chars_per_token.trim().parse::<f64>().unwrap_or(4.0);
    let head_ratio = head_ratio.trim().parse::<f64>().unwrap_or(0.5);
    let keep = Keep::parse(keep).map_err(|e| JsValue::from_str(&e))?;
    gizza_ai_context_trimmer_core::trim(
        text,
        max_tokens,
        chars_per_token,
        keep,
        marker,
        head_ratio,
        truthy(break_words),
    )
    .map_err(|e| JsValue::from_str(&e))
}
