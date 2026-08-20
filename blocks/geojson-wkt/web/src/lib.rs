//! Browser-facing wasm-bindgen wrapper for /tools/geojson-wkt/.
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(
    input: &str,
    from: &str,
    to: &str,
    multi: &str,
    srid: &str,
    precision: &str,
    wkb_encoding: &str,
    wkb_endian: &str,
    pretty: &str,
) -> Result<String, JsValue> {
    let truthy = matches!(
        pretty.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "yes" | "on"
    );
    gizza_ai_geojson_wkt_core::convert(
        input,
        from,
        to,
        multi,
        srid.trim().parse::<i64>().unwrap_or(0),
        precision.trim().parse::<i64>().unwrap_or(-1),
        wkb_encoding,
        wkb_endian,
        truthy,
    )
    .map_err(|e| JsValue::from_str(&e))
}
