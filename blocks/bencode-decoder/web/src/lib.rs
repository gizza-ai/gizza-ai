//! Browser-facing wasm-bindgen wrapper for /tools/bencode-decoder/.
//! Field order MUST match meta.toml: input, mode, pretty.
use gizza_ai_bencode_decoder_core::{decode_to_json, encode_from_json};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(input: &str, mode: &str, pretty: &str) -> Result<String, JsValue> {
    let pretty = matches!(pretty.trim(), "true" | "1" | "on" | "yes");
    match mode.trim().to_ascii_lowercase().as_str() {
        "encode" => {
            let bytes = encode_from_json(input).map_err(|e| JsValue::from_str(&e))?;
            Ok(String::from_utf8_lossy(&bytes).into_owned())
        }
        // decode is the default for any other / empty mode value.
        _ => decode_to_json(input.as_bytes(), pretty).map_err(|e| JsValue::from_str(&e)),
    }
}
