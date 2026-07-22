//! Browser-facing wasm-bindgen wrapper for /tools/before-after-slider/.
//! Field order MUST match meta.toml: before, after, before_label, after_label,
//! orientation, start_position, width, move_on_hover, handle_color, output.
use gizza_ai_before_after_slider_core::{parse_orientation, parse_output, render, Options};
use wasm_bindgen::prelude::*;

fn parse_f64(s: &str, default: f64) -> f64 {
    match s.trim() {
        "" => default,
        t => t.parse::<f64>().unwrap_or(default),
    }
}

fn parse_u32(s: &str, default: u32) -> u32 {
    match s.trim() {
        "" => default,
        t => t.parse::<u32>().unwrap_or(default),
    }
}

fn truthy(s: &str) -> bool {
    matches!(s.trim().to_ascii_lowercase().as_str(), "true" | "1" | "on" | "yes")
}

#[wasm_bindgen]
#[allow(clippy::too_many_arguments)]
pub fn run(
    before: &str,
    after: &str,
    before_label: &str,
    after_label: &str,
    orientation: &str,
    start_position: &str,
    width: &str,
    move_on_hover: &str,
    handle_color: &str,
    output: &str,
) -> Result<String, JsValue> {
    let opts = Options {
        before: before.to_string(),
        after: after.to_string(),
        before_label: before_label.to_string(),
        after_label: after_label.to_string(),
        orientation: parse_orientation(orientation).map_err(|e| JsValue::from_str(&e))?,
        start: parse_f64(start_position, 50.0),
        width: parse_u32(width, 0),
        move_on_hover: truthy(move_on_hover),
        handle_color: if handle_color.trim().is_empty() {
            "#ffffff".to_string()
        } else {
            handle_color.to_string()
        },
        output: parse_output(output).map_err(|e| JsValue::from_str(&e))?,
    };
    render(&opts).map_err(|e| JsValue::from_str(&e))
}
