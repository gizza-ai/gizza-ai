//! Browser-facing wasm-bindgen wrapper for /tools/audio-feature-csv-exporter/.
//! Field order MUST match meta.toml. Every field arrives as a string
//! (checkboxes send "true"/"false"); blank numerics fall back to the same
//! defaults the descriptor declares.
use gizza_ai_audio_feature_csv_exporter_core::{run as export, Options};
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
    frame_ms: &str,
    hop_ms: &str,
    window: &str,
    center: &str,
    channel: &str,
    resample_hz: &str,
    rms: &str,
    centroid: &str,
    zcr: &str,
    rolloff: &str,
    flatness: &str,
    bandwidth: &str,
    flux: &str,
    rolloff_percent: &str,
    rms_scale: &str,
    flatness_scale: &str,
    include_time: &str,
    include_frame: &str,
    decimals: &str,
) -> Result<String, JsValue> {
    let opts = Options {
        output: or_default(output, "csv"),
        frame_ms: parse_f64(frame_ms, 25.0),
        hop_ms: parse_f64(hop_ms, 10.0),
        window: or_default(window, "hann"),
        center: truthy(center),
        channel: or_default(channel, "mix"),
        resample_hz: parse_i64(resample_hz, 0),
        rms: truthy(rms),
        centroid: truthy(centroid),
        zcr: truthy(zcr),
        rolloff: truthy(rolloff),
        flatness: truthy(flatness),
        bandwidth: truthy(bandwidth),
        flux: truthy(flux),
        rolloff_percent: parse_f64(rolloff_percent, 85.0),
        rms_scale: or_default(rms_scale, "dbfs"),
        flatness_scale: or_default(flatness_scale, "ratio"),
        include_time: truthy(include_time),
        include_frame: truthy(include_frame),
        decimals: parse_i64(decimals, 6),
    };
    export(input, &or_default(input_format, "base64"), &opts).map_err(|e| JsValue::from_str(&e))
}
