//! Browser-facing wasm-bindgen wrapper for /tools/gpx-to-kml/.
//! Field order MUST match meta.toml: gpx, line_color, line_width, line_opacity,
//! waypoint_color, altitude_mode, document_name. Fields arrive as strings.
use gizza_ai_gpx_to_kml_core::{convert, AltitudeMode, Options};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(
    gpx: &str,
    line_color: &str,
    line_width: &str,
    line_opacity: &str,
    waypoint_color: &str,
    altitude_mode: &str,
    document_name: &str,
) -> Result<String, JsValue> {
    let line_width = line_width.trim().parse::<u32>().unwrap_or(4);
    let line_opacity = line_opacity.trim().parse::<u32>().unwrap_or(80);
    let line_color = if line_color.trim().is_empty() {
        "#ef4444"
    } else {
        line_color
    };
    let waypoint_color = if waypoint_color.trim().is_empty() {
        "#3b82f6"
    } else {
        waypoint_color
    };
    let altitude_mode = AltitudeMode::parse(altitude_mode).map_err(|e| JsValue::from_str(&e))?;
    let opt = Options {
        line_color: line_color.to_string(),
        line_width,
        line_opacity,
        waypoint_color: waypoint_color.to_string(),
        altitude_mode,
        document_name: Some(document_name.to_string()),
    };
    convert(gpx, &opt).map_err(|e| JsValue::from_str(&e))
}
