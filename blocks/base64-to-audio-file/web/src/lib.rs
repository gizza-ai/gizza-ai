//! Browser-facing wasm-bindgen wrapper for /tools/base64-to-audio-file/.
//! Compiled with wasm-pack for the standalone page.
use wasm_bindgen::prelude::*;

/// Decode Base64 (or a `data:` URI) into audio and return it as a
/// `data:<mime>;base64,…` URL — the page renders that string, and it can be
/// played, saved, or pasted straight into the address bar. Throws a JS error
/// string when the payload is not valid Base64 or (under `strict`) not audio.
#[wasm_bindgen]
pub fn run(data: &str, filename: &str, format: &str, strict: &str) -> Result<String, JsValue> {
    let strict = match strict.trim().to_ascii_lowercase().as_str() {
        "" => true,
        "false" | "0" | "off" | "no" => false,
        _ => true,
    };
    let format = if format.trim().is_empty() {
        "auto"
    } else {
        format
    };
    gizza_ai_base64_to_audio_file_core::render(data, filename, format, strict)
        .map_err(|e| JsValue::from_str(&e))
}
