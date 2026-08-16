//! Browser-facing wasm-bindgen wrapper for /tools/tempo-map-extractor/.
//! The page hands every field over as a raw string; a blank field means "use
//! the default", which is what the core would apply anyway.
use gizza_ai_tempo_map_extractor_core::{extract, Spec};
use wasm_bindgen::prelude::*;

fn parse_usize(v: &str, default: usize, name: &str) -> Result<usize, JsValue> {
    let t = v.trim();
    if t.is_empty() {
        Ok(default)
    } else {
        t.parse::<usize>()
            .map_err(|_| JsValue::from_str(&format!("{name} must be a whole number, got '{t}'")))
    }
}

fn parse_f64(v: &str, default: f64, name: &str) -> Result<f64, JsValue> {
    let t = v.trim();
    if t.is_empty() {
        Ok(default)
    } else {
        t.parse::<f64>()
            .map_err(|_| JsValue::from_str(&format!("{name} must be a number, got '{t}'")))
    }
}

fn or_default<'a>(v: &'a str, default: &'a str) -> &'a str {
    if v.trim().is_empty() {
        default
    } else {
        v
    }
}

#[allow(clippy::too_many_arguments)]
#[wasm_bindgen]
pub fn run(
    beats: &str,
    time_unit: &str,
    fps: &str,
    beat_unit: &str,
    smoothing: &str,
    smooth_method: &str,
    grid_seconds: &str,
    min_interval_ms: &str,
    offset_seconds: &str,
    decimals: &str,
    output: &str,
    ppq: &str,
) -> Result<String, JsValue> {
    let spec = Spec {
        beats,
        time_unit: or_default(time_unit, "auto"),
        fps: parse_f64(fps, 30.0, "fps")?,
        beat_unit: or_default(beat_unit, "quarter"),
        smoothing: parse_usize(smoothing, 1, "smoothing")?,
        smooth_method: or_default(smooth_method, "mean"),
        grid_seconds: parse_f64(grid_seconds, 0.0, "grid_seconds")?,
        min_interval_ms: parse_f64(min_interval_ms, 0.0, "min_interval_ms")?,
        offset_seconds: parse_f64(offset_seconds, 0.0, "offset_seconds")?,
        decimals: parse_usize(decimals, 2, "decimals")?,
        output: or_default(output, "csv"),
        ppq: parse_usize(ppq, 960, "ppq")?,
    };
    extract(&spec).map_err(|e| JsValue::from_str(&e))
}
