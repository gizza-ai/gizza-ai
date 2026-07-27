//! Browser-facing wasm-bindgen wrapper for /tools/emoji-remover/.
//! Field order MUST match meta.toml: text, mode, placeholder,
//! collapse_whitespace, keep_text_symbols.
use gizza_ai_emoji_remover_core::{remove_emoji, Mode};
use wasm_bindgen::prelude::*;

fn truthy(v: &str) -> bool {
    matches!(
        v.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "on" | "yes"
    )
}

#[wasm_bindgen]
pub fn run(
    text: &str,
    mode: &str,
    placeholder: &str,
    collapse_whitespace: &str,
    keep_text_symbols: &str,
) -> Result<String, JsValue> {
    let mode_value = if mode.trim().is_empty() {
        "remove"
    } else {
        mode
    };
    let mode = Mode::parse(mode_value).ok_or_else(|| {
        JsValue::from_str(&format!(
            "expected mode to be 'remove', 'space', or 'placeholder', got '{mode_value}'"
        ))
    })?;
    Ok(remove_emoji(
        text,
        mode,
        placeholder,
        truthy(collapse_whitespace),
        truthy(keep_text_symbols),
    ))
}
