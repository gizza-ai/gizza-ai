//! gizza-ai/utm-to-latlon-csv — convert CSV UTM coordinates to WGS84 latitude/longitude and back.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_utm_to_latlon_csv_core::convert;
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    input: String,
    #[serde(default)]
    direction: String,
    #[serde(default)]
    easting_column: String,
    #[serde(default)]
    northing_column: String,
    #[serde(default)]
    zone_column: String,
    #[serde(default)]
    zone: String,
    #[serde(default)]
    hemisphere: String,
    #[serde(default)]
    coord_format: String,
    #[serde(default = "default_decimals")]
    decimals: i64,
    #[serde(default)]
    ellipsoid: String,
    #[serde(default)]
    delimiter: String,
    #[serde(default = "default_true")]
    has_header: bool,
    #[serde(default = "default_true")]
    keep_columns: bool,
    #[serde(default = "default_true")]
    validate_ranges: bool,
    #[serde(default)]
    output: String,
}

fn default_decimals() -> i64 {
    6
}
fn default_true() -> bool {
    true
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("input").required().describe("CSV or delimited text containing UTM easting, northing and zone columns (or latitude and longitude columns when direction=latlon_to_utm). The first row is treated as a header by default."))
        .param(Param::enumv("direction", ["utm_to_latlon", "latlon_to_utm"]).default("utm_to_latlon").describe("Conversion direction. Use utm_to_latlon to add latitude/longitude columns from UTM metres, or latlon_to_utm to add easting/northing/zone columns from WGS84 latitude/longitude."))
        .param(Param::string("easting_column").default("").describe("UTM easting column for utm_to_latlon, or latitude column for latlon_to_utm. Accepts a header name or 1-based column number. Blank auto-detects common names, then falls back to column 1."))
        .param(Param::string("northing_column").default("").describe("UTM northing column for utm_to_latlon, or longitude column for latlon_to_utm. Accepts a header name or 1-based column number. Blank auto-detects common names, then falls back to column 2."))
        .param(Param::string("zone_column").default("").describe("Optional UTM zone column name or 1-based index. Values may be zone numbers, MGRS-style bands such as 18T, hemisphere suffixes such as 56S, or EPSG codes such as EPSG:32618."))
        .param(Param::string("zone").default("").describe("Fallback or forced UTM zone when a row has no zone column. Examples: 18, 18T, 18N, EPSG:32618. For latlon_to_utm, leave blank to pick the standard zone from longitude."))
        .param(Param::enumv("hemisphere", ["auto", "north", "south"]).default("auto").describe("Hemisphere interpretation for UTM northings. Auto uses the latitude-band letter or EPSG code when present and otherwise assumes north; choose north or south to override a questionable zone label."))
        .param(Param::enumv("coord_format", ["decimal", "dms", "ddm"]).default("decimal").describe("Latitude/longitude format for utm_to_latlon output: decimal degrees, degrees-minutes-seconds (dms), or degrees-decimal-minutes (ddm)."))
        .param(Param::integer("decimals").default(6).min(0.0).max(12.0).describe("Number of decimal places to print for numeric output. For DMS/DDM, the setting controls equivalent precision and is capped at 12."))
        .param(Param::enumv("ellipsoid", ["wgs84", "grs80", "clarke1866", "international1924"]).default("wgs84").describe("Reference ellipsoid used for the Transverse Mercator math. This does not apply a datum shift; it only changes the ellipsoid shape."))
        .param(Param::enumv("delimiter", ["auto", "comma", "semicolon", "tab", "pipe"]).default("auto").describe("Input delimiter. Auto sniffs the first non-blank line and preserves the detected delimiter for CSV output."))
        .param(Param::boolean("has_header").default(true).describe("Treat the first non-blank row as column headers. When false, columns are named column1, column2, and so on."))
        .param(Param::boolean("keep_columns").default(true).describe("Keep non-coordinate columns in the output. Disable this to return only the converted coordinate columns."))
        .param(Param::boolean("validate_ranges").default(true).describe("Reject out-of-range UTM eastings/northings, latitude outside UTM coverage, and latitude-band mismatches before returning output."))
        .param(Param::enumv("output", ["csv", "tsv", "json", "table", "geojson", "kml"]).default("csv").describe("Output format. CSV/TSV are spreadsheet-friendly, JSON/table are for inspection, and GeoJSON/KML create point features from the converted coordinates."))
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/utm-to-latlon-csv",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Convert UTM coordinate CSV files to latitude/longitude, and back.",
    skill(
        description = "Convert CSV rows between UTM easting/northing/zone coordinates and WGS84 latitude/longitude. Auto-detects common coordinate column names, accepts UTM zones as numbers, MGRS latitude bands or EPSG:326xx/327xx codes, can reverse latitude/longitude rows back to UTM, and writes CSV, TSV, JSON, aligned tables, GeoJSON or KML. Runs offline with a pure Rust Transverse Mercator implementation; ellipsoid choices do not perform datum shifts.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "utm-to-latlon-csv", |a: Args| {
            convert(
                &a.input,
                &a.direction,
                &a.easting_column,
                &a.northing_column,
                &a.zone_column,
                &a.zone,
                &a.hemisphere,
                &a.coord_format,
                a.decimals,
                &a.ellipsoid,
                &a.delimiter,
                a.has_header,
                a.keep_columns,
                a.validate_ranges,
                &a.output,
            )
            .map_err(SkillError::InvalidArgs)
        }) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_json_exposes_real_parameters() {
        let schema: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        let props = schema.get("properties").and_then(|v| v.as_object()).unwrap();
        for name in [
            "input",
            "direction",
            "easting_column",
            "northing_column",
            "zone_column",
            "zone",
            "hemisphere",
            "coord_format",
            "decimals",
            "ellipsoid",
            "delimiter",
            "has_header",
            "keep_columns",
            "validate_ranges",
            "output",
        ] {
            assert!(props.contains_key(name), "missing {name}");
            assert!(props[name].get("description").is_some(), "missing description for {name}");
        }
        assert_eq!(schema.get("required").unwrap(), &serde_json::json!(["input"]));
    }
}
