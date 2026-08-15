//! Browser-facing wasm-bindgen wrapper for /tools/kml-to-geojson/.
//! Field order MUST match meta.toml: input, output_format, include_styles,
//! include_folders, precision, document_name, altitude_mode. Fields arrive as
//! strings (checkboxes send "true"/"false"); blanks fall back to the defaults
//! the descriptor advertises, and `core` owns all validation.
use gizza_ai_kml_to_geojson_core::{convert, AltitudeMode, Options, OutputFormat};
use wasm_bindgen::prelude::*;

fn truthy(s: &str) -> bool {
    matches!(s.trim().to_ascii_lowercase().as_str(), "true" | "1" | "on" | "yes")
}

#[wasm_bindgen]
pub fn run(
    input: &str,
    output_format: &str,
    include_styles: &str,
    include_folders: &str,
    precision: &str,
    document_name: &str,
    altitude_mode: &str,
) -> Result<String, JsValue> {
    let fmt = if output_format.trim().is_empty() { "geojson" } else { output_format };
    let output_format = OutputFormat::parse(fmt).map_err(|e| JsValue::from_str(&e))?;
    let mode = if altitude_mode.trim().is_empty() { "clamp_to_ground" } else { altitude_mode };
    let altitude_mode = AltitudeMode::parse(mode).map_err(|e| JsValue::from_str(&e))?;
    let precision = match precision.trim() {
        "" => 6,
        other => other
            .parse::<u32>()
            .map_err(|_| JsValue::from_str("precision must be a whole number between 0 and 15"))?,
    };
    let opt = Options {
        output_format,
        include_styles: truthy(include_styles),
        include_folders: truthy(include_folders),
        precision,
        document_name: document_name.to_string(),
        altitude_mode,
    };
    convert(input, &opt).map_err(|e| JsValue::from_str(&e))
}
