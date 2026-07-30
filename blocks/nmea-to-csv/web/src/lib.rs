//! Browser-facing wasm-bindgen wrapper for /tools/nmea-to-csv/.
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(
    nmea: &str,
    coordinates: &str,
    altitude_unit: &str,
    speed_unit: &str,
    delimiter: &str,
    header: bool,
    validate_checksum: bool,
) -> Result<String, JsValue> {
    gizza_ai_nmea_to_csv_core::convert_str(
        nmea,
        coordinates,
        altitude_unit,
        speed_unit,
        delimiter,
        header,
        validate_checksum,
    )
    .map_err(|e| JsValue::from_str(&e))
}
