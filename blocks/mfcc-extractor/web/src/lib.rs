//! Browser-facing wasm-bindgen wrapper for /tools/mfcc-extractor/.
//! Field order MUST match meta.toml. Every field arrives as a string
//! (checkboxes send "true"/"false"); blank numerics fall back to the same
//! defaults the descriptor declares.
use gizza_ai_mfcc_extractor_core::{run as extract, Options};
use wasm_bindgen::prelude::*;

fn truthy(s: &str) -> bool {
    matches!(
        s.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "on" | "yes"
    )
}

fn parse_f64(s: &str, default: f64) -> f64 {
    let t = s.trim();
    if t.is_empty() {
        default
    } else {
        t.parse().unwrap_or(default)
    }
}

fn parse_i64(s: &str, default: i64) -> i64 {
    let t = s.trim();
    if t.is_empty() {
        default
    } else {
        // A slider/number field can hand back "16000.0"; take the integer part.
        t.parse::<i64>()
            .or_else(|_| t.parse::<f64>().map(|v| v.round() as i64))
            .unwrap_or(default)
    }
}

fn or_default(s: &str, default: &str) -> String {
    let t = s.trim();
    if t.is_empty() {
        default.to_string()
    } else {
        t.to_string()
    }
}

#[wasm_bindgen]
#[allow(clippy::too_many_arguments)]
pub fn run(
    input: &str,
    input_format: &str,
    output: &str,
    n_mfcc: &str,
    n_mels: &str,
    frame_ms: &str,
    hop_ms: &str,
    fmin: &str,
    fmax: &str,
    window: &str,
    preemphasis: &str,
    lifter: &str,
    mel_scale: &str,
    append_energy: &str,
    deltas: &str,
    include_time: &str,
    decimals: &str,
    resample_hz: &str,
) -> Result<String, JsValue> {
    let opts = Options {
        output: or_default(output, "csv"),
        n_mfcc: parse_i64(n_mfcc, 13),
        n_mels: parse_i64(n_mels, 26),
        frame_ms: parse_f64(frame_ms, 25.0),
        hop_ms: parse_f64(hop_ms, 10.0),
        fmin: parse_f64(fmin, 0.0),
        fmax: parse_f64(fmax, 0.0),
        window: or_default(window, "hamming"),
        preemphasis: parse_f64(preemphasis, 0.97),
        lifter: parse_f64(lifter, 22.0),
        mel_scale: or_default(mel_scale, "htk"),
        append_energy: truthy(append_energy),
        deltas: or_default(deltas, "none"),
        include_time: truthy(include_time),
        decimals: parse_i64(decimals, 6),
        resample_hz: parse_i64(resample_hz, 0),
    };
    extract(input, &or_default(input_format, "base64"), &opts).map_err(|e| JsValue::from_str(&e))
}
