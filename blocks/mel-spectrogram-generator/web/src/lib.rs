//! Browser-facing wasm-bindgen wrapper for /tools/mel-spectrogram-generator/.
//! Field order MUST match page/meta.toml. The file input arrives as base64.
use gizza_ai_mel_spectrogram_generator_core::{run as render, Options};
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

fn base64(bytes: &[u8]) -> String {
    const A: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = *chunk.get(1).unwrap_or(&0) as u32;
        let b2 = *chunk.get(2).unwrap_or(&0) as u32;
        let n = (b0 << 16) | (b1 << 8) | b2;
        out.push(A[((n >> 18) & 63) as usize] as char);
        out.push(A[((n >> 12) & 63) as usize] as char);
        out.push(if chunk.len() > 1 {
            A[((n >> 6) & 63) as usize] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            A[(n & 63) as usize] as char
        } else {
            '='
        });
    }
    out
}

#[wasm_bindgen]
#[allow(clippy::too_many_arguments)]
pub fn run(
    input: &str,
    input_format: &str,
    n_fft: &str,
    hop_length: &str,
    n_mels: &str,
    fmin: &str,
    fmax: &str,
    mel_scale: &str,
    window: &str,
    scale: &str,
    top_db: &str,
    db_reference: &str,
    colormap: &str,
    width: &str,
    height: &str,
    channel: &str,
    center: &str,
    resample_hz: &str,
) -> Result<String, JsValue> {
    let opts = Options {
        n_fft: parse_i64(n_fft, 2048),
        hop_length: parse_i64(hop_length, 0),
        n_mels: parse_i64(n_mels, 128),
        fmin: parse_f64(fmin, 0.0),
        fmax: parse_f64(fmax, 0.0),
        mel_scale: or_default(mel_scale, "slaney"),
        window: or_default(window, "hann"),
        scale: or_default(scale, "db"),
        top_db: parse_f64(top_db, 80.0),
        db_reference: or_default(db_reference, "peak"),
        colormap: or_default(colormap, "magma"),
        width: parse_i64(width, 1000),
        height: parse_i64(height, 400),
        channel: or_default(channel, "mix"),
        center: truthy(center),
        resample_hz: parse_i64(resample_hz, 0),
    };
    let r = render(input, &or_default(input_format, "base64"), &opts)
        .map_err(|e| JsValue::from_str(&e))?;
    let data_url = format!("data:image/png;base64,{}", base64(&r.png));
    Ok(format!(
        "{{\"dataUrl\":\"{}\",\"summary\":{},\"width\":{},\"height\":{},\"frames\":{},\"n_mels\":{}}}",
        data_url,
        serde_json::to_string(&r.summary).map_err(|e| JsValue::from_str(&e.to_string()))?,
        r.width,
        r.height,
        r.frames,
        r.n_mels
    ))
}
