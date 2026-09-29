//! Browser-facing wasm-bindgen wrapper for /tools/video-bitrate-checker/.
//!
//! Argument order MUST match page/meta.toml. The chosen file arrives as the
//! base64 payload page/custom.js reads off the file input; every other field
//! arrives as a string, and a blank one falls back to the same default the
//! descriptor declares. Only the container is demuxed — no decoding — so the
//! bytes never leave the tab.
use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use gizza_ai_video_bitrate_checker_core::{analyze_json, Options, Target, Units};
use wasm_bindgen::prelude::*;

/// Parse an optional number field, falling back to `default` when blank.
fn parse_f64(s: &str, default: f64, label: &str) -> Result<f64, String> {
    let t = s.trim();
    if t.is_empty() {
        return Ok(default);
    }
    t.parse::<f64>()
        .map_err(|_| format!("{label} must be a number, got {t:?}"))
}

/// Accept both a raw base64 payload and a `data:video/mp4;base64,…` URL, and
/// tolerate the newlines a pasted payload can carry.
fn decode_media(input: &str) -> Result<Vec<u8>, String> {
    let payload = match input.find(";base64,") {
        Some(i) => &input[i + ";base64,".len()..],
        None => input,
    };
    let cleaned: String = payload.chars().filter(|c| !c.is_whitespace()).collect();
    if cleaned.is_empty() {
        return Err("no file data was provided".into());
    }
    STANDARD
        .decode(cleaned.as_bytes())
        .map_err(|e| format!("could not read the selected file: {e}"))
}

#[wasm_bindgen]
pub fn run(
    media: &str,
    min_bitrate: &str,
    max_bitrate: &str,
    units: &str,
    target: &str,
) -> Result<String, JsValue> {
    let d = Options::default();
    let opts = Options {
        min_bitrate: parse_f64(min_bitrate, d.min_bitrate, "min_bitrate")
            .map_err(|e| JsValue::from_str(&e))?,
        max_bitrate: parse_f64(max_bitrate, d.max_bitrate, "max_bitrate")
            .map_err(|e| JsValue::from_str(&e))?,
        units: if units.trim().is_empty() {
            d.units
        } else {
            Units::parse(units).map_err(|e| JsValue::from_str(&e))?
        },
        target: if target.trim().is_empty() {
            d.target
        } else {
            Target::parse(target).map_err(|e| JsValue::from_str(&e))?
        },
    };
    // Reject an impossible range before decoding megabytes of base64.
    opts.validate().map_err(|e| JsValue::from_str(&e))?;
    let bytes = decode_media(media).map_err(|e| JsValue::from_str(&e))?;
    analyze_json(bytes, &opts).map_err(|e| JsValue::from_str(&e))
}
