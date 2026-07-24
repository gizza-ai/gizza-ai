//! Browser-facing wasm-bindgen wrapper for /tools/gpx-split/.
use gizza_ai_gpx_split_core::{render, Config, Mode, Output, Unit};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(
    gpx: &str,
    mode: &str,
    distance: f64,
    unit: &str,
    time_min: f64,
    stop_gap_s: f64,
    output: &str,
) -> Result<String, JsValue> {
    let cfg = Config {
        mode: Mode::parse(if mode.trim().is_empty() { "distance" } else { mode })
            .map_err(|e| JsValue::from_str(&e))?,
        distance: if distance <= 0.0 { 5.0 } else { distance },
        unit: Unit::parse(if unit.trim().is_empty() { "km" } else { unit })
            .map_err(|e| JsValue::from_str(&e))?,
        time_min: if time_min <= 0.0 { 30.0 } else { time_min },
        stop_gap_s: if stop_gap_s <= 0.0 { 120.0 } else { stop_gap_s },
        output: Output::parse(if output.trim().is_empty() { "gpx" } else { output })
            .map_err(|e| JsValue::from_str(&e))?,
    };
    render(gpx, &cfg).map_err(|e| JsValue::from_str(&e))
}
