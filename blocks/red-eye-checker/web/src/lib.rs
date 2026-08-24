//! Browser-facing wasm-bindgen wrapper for /tools/red-eye-checker/.
//! Argument order MUST match page/meta.toml. The uploaded image arrives as the
//! base64 payload page/custom.js reads off the file input; every other field
//! arrives as a string, and a blank one falls back to the same default the
//! descriptor declares.
use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use gizza_ai_red_eye_checker_core::{analyze_json, Options, Sensitivity};
use wasm_bindgen::prelude::*;

fn parse_u32(s: &str, default: u32) -> u32 {
    let t = s.trim();
    if t.is_empty() {
        return default;
    }
    // A slider can hand back "20.0"; take the integer part.
    t.parse::<u32>()
        .or_else(|_| t.parse::<f64>().map(|v| v.round().max(0.0) as u32))
        .unwrap_or(default)
}

/// Accept both a raw base64 payload and a `data:image/png;base64,…` URL, and
/// tolerate the newlines a pasted payload can carry.
fn decode_image(input: &str) -> Result<Vec<u8>, String> {
    let payload = match input.find(";base64,") {
        Some(i) => &input[i + ";base64,".len()..],
        None => input,
    };
    let cleaned: String = payload.chars().filter(|c| !c.is_whitespace()).collect();
    if cleaned.is_empty() {
        return Err("no image data was provided".into());
    }
    STANDARD
        .decode(cleaned.as_bytes())
        .map_err(|e| format!("could not read the selected image: {e}"))
}

#[wasm_bindgen]
pub fn run(
    image: &str,
    sensitivity: &str,
    min_radius: &str,
    max_radius: &str,
    max_regions: &str,
) -> Result<String, JsValue> {
    let d = Options::default();
    let opts = Options {
        sensitivity: Sensitivity::parse(sensitivity).map_err(|e| JsValue::from_str(&e))?,
        min_radius: parse_u32(min_radius, d.min_radius),
        max_radius: parse_u32(max_radius, d.max_radius),
        max_regions: parse_u32(max_regions, d.max_regions),
    };
    let bytes = decode_image(image).map_err(|e| JsValue::from_str(&e))?;
    analyze_json(&bytes, &opts).map_err(|e| JsValue::from_str(&e))
}
