//! Browser-facing wasm-bindgen wrapper for /tools/owntracks-to-gpx/.
//! Field order MUST match meta.toml: input, track_name, include_extensions,
//! segment_gap_minutes, max_accuracy_meters. Fields arrive as strings (checkboxes
//! send "true"/"false"; numbers arrive as numeric-looking strings).
use gizza_ai_owntracks_to_gpx_core::{convert, Options};
use wasm_bindgen::prelude::*;

fn truthy(s: &str) -> bool {
    matches!(s.trim().to_ascii_lowercase().as_str(), "true" | "1" | "on" | "yes")
}

/// Parse a numeric field, treating blank/unparseable as 0 (the "unset" sentinel).
fn num(s: &str) -> f64 {
    s.trim().parse::<f64>().unwrap_or(0.0)
}

#[wasm_bindgen]
pub fn run(
    input: &str,
    track_name: &str,
    include_extensions: &str,
    segment_gap_minutes: &str,
    max_accuracy_meters: &str,
) -> Result<String, JsValue> {
    let opt = Options {
        track_name: track_name.trim().to_string(),
        include_extensions: truthy(include_extensions),
        segment_gap_minutes: num(segment_gap_minutes),
        max_accuracy_meters: num(max_accuracy_meters),
    };
    convert(input, &opt).map_err(|e| JsValue::from_str(&e))
}
