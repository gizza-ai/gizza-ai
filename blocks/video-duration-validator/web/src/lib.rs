//! Browser-facing wasm-bindgen wrapper for /tools/video-duration-validator/.
//!
//! Argument order MUST match page/meta.toml. The chosen video arrives as the
//! base64 payload page/custom.js reads off the file input; every other field
//! arrives as a string, and a blank one falls back to the same default the
//! descriptor declares. Only the container header is parsed, so the bytes never
//! leave the tab.
use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use gizza_ai_video_duration_validator_core::{validate_json, Mode, Options};
use wasm_bindgen::prelude::*;

/// Parse an optional number field, falling back to `default` when blank.
fn parse_f64(s: &str, default: f64, label: &str) -> Result<f64, String> {
    let t = s.trim();
    if t.is_empty() {
        return Ok(default);
    }
    t.parse::<f64>()
        .map_err(|_| format!("{label} must be a number of seconds, got {t:?}"))
}

/// Accept both a raw base64 payload and a `data:video/mp4;base64,…` URL, and
/// tolerate the newlines a pasted payload can carry.
fn decode_video(input: &str) -> Result<Vec<u8>, String> {
    let payload = match input.find(";base64,") {
        Some(i) => &input[i + ";base64,".len()..],
        None => input,
    };
    let cleaned: String = payload.chars().filter(|c| !c.is_whitespace()).collect();
    if cleaned.is_empty() {
        return Err("no video data was provided".into());
    }
    STANDARD
        .decode(cleaned.as_bytes())
        .map_err(|e| format!("could not read the selected video: {e}"))
}

#[wasm_bindgen]
pub fn run(
    video: &str,
    target_seconds: &str,
    tolerance_seconds: &str,
    mode: &str,
) -> Result<String, JsValue> {
    let d = Options::default();
    if target_seconds.trim().is_empty() {
        return Err(JsValue::from_str(
            "enter the target length in seconds (for example 30)",
        ));
    }
    let opts = Options {
        target_seconds: parse_f64(target_seconds, 0.0, "target_seconds")
            .map_err(|e| JsValue::from_str(&e))?,
        tolerance_seconds: parse_f64(tolerance_seconds, d.tolerance_seconds, "tolerance_seconds")
            .map_err(|e| JsValue::from_str(&e))?,
        mode: if mode.trim().is_empty() {
            d.mode
        } else {
            Mode::parse(mode).map_err(|e| JsValue::from_str(&e))?
        },
    };
    // Reject a bad rule before decoding megabytes of base64.
    opts.validate().map_err(|e| JsValue::from_str(&e))?;
    let bytes = decode_video(video).map_err(|e| JsValue::from_str(&e))?;
    validate_json(&bytes, &opts).map_err(|e| JsValue::from_str(&e))
}
