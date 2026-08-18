//! gizza-ai/csv-to-geojson — convert tabular coordinates to GeoJSON.
//! Chat schema is single-sourced from descriptor() (also drives the CLI);
//! handle() delegates to the pure core converter.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_csv_to_geojson_core::{convert, Options};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    input: String,
    #[serde(default)]
    lat: String,
    #[serde(default)]
    lon: String,
    #[serde(default)]
    elevation: String,
    #[serde(default = "default_delimiter")]
    delimiter: String,
    #[serde(default = "default_shape")]
    shape: String,
    #[serde(default = "default_types")]
    types: String,
    #[serde(default)]
    precision: i64,
    #[serde(default = "default_invalid")]
    invalid: String,
    #[serde(default)]
    bbox: bool,
    #[serde(default = "default_pretty")]
    pretty: bool,
}

fn default_delimiter() -> String {
    "auto".into()
}
fn default_shape() -> String {
    "points".into()
}
fn default_types() -> String {
    "infer".into()
}
fn default_invalid() -> String {
    "skip".into()
}
fn default_pretty() -> bool {
    true
}

/// Single source for the chat schema (and CLI). Edit the params to match the
/// tool's real inputs — e.g. `.param(Param::enumv("mode", ["a","b"]).default("a"))`,
/// `.param(Param::integer("n").min(1.0))`. Use Input::Image/Video/Document/File
/// for tools that take a url/ref media input (see image-resize / web-fetch).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("input").required().describe("CSV/TSV/pipe/semicolon table with a header row, or a JSON array of objects. The table must contain latitude and longitude columns (auto-detected from common names such as lat/lon, latitude/longitude, y/x) unless you set lat and lon explicitly."))
        .param(Param::string("lat").default("").describe("Latitude column name or 1-based column index. Leave blank to auto-detect from common headers such as lat, latitude, y, northing, lat_dd."))
        .param(Param::string("lon").default("").describe("Longitude column name or 1-based column index. Leave blank to auto-detect from common headers such as lon, lng, longitude, x, easting, lon_dd."))
        .param(Param::string("elevation").default("").describe("Optional elevation/altitude column name or 1-based index. Leave blank to auto-detect common altitude headers, or set '-' to keep elevation as a normal property instead of a third coordinate."))
        .param(Param::enumv("delimiter", ["auto", "comma", "semicolon", "tab", "pipe"]).default("auto").describe("Input delimiter for text tables. 'auto' sniffs the header row; use comma, semicolon, tab, or pipe to force a delimiter. JSON input ignores this."))
        .param(Param::enumv("shape", ["points", "line", "polygon"]).default("points").describe("GeoJSON shape to create. 'points' returns a FeatureCollection of Point features; 'line' joins rows in order into one LineString feature; 'polygon' joins rows into one closed Polygon ring."))
        .param(Param::enumv("types", ["infer", "string"]).default("infer").describe("How non-coordinate properties are typed. 'infer' converts plain numbers, booleans and blanks to JSON values while preserving identifiers like leading-zero ZIP codes; 'string' keeps CSV cells as strings."))
        .param(Param::integer("precision").min(0.0).max(15.0).default(0).describe("Decimal places for coordinates and elevation. 0 keeps full parsed precision; 1-15 rounds coordinates after validation."))
        .param(Param::enumv("invalid", ["skip", "error", "null"]).default("skip").describe("How to handle rows with missing, non-numeric, or out-of-range coordinates. 'skip' omits them, 'error' stops with the first row error, and 'null' keeps a Feature with null geometry and the row properties."))
        .param(Param::boolean("bbox").default(false).describe("Add a GeoJSON bbox array spanning all valid coordinates. Default false."))
        .param(Param::boolean("pretty").default(true).describe("Pretty-print GeoJSON with two-space indentation. Disable for compact one-line JSON."))
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/csv-to-geojson",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Convert CSV or JSON latitude/longitude rows into GeoJSON",
    skill(
        description = "Convert a CSV/TSV/semicolon/pipe table or JSON array of objects with latitude and longitude columns into RFC 7946 GeoJSON. It auto-detects common lat/lon headers, preserves non-coordinate columns as properties, can infer property types, rounds coordinates, adds bbox, keeps invalid rows as null geometries, and can join rows into points, a LineString, or a Polygon.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "csv-to-geojson", |a: Args| {
            convert(
                &a.input,
                &Options {
                    lat: a.lat,
                    lon: a.lon,
                    elevation: a.elevation,
                    delimiter: a.delimiter,
                    shape: a.shape,
                    types: a.types,
                    precision: a.precision,
                    invalid: a.invalid,
                    bbox: a.bbox,
                    pretty: a.pretty,
                },
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
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "input": { "type": "string", "description": "CSV/TSV/pipe/semicolon table with a header row, or a JSON array of objects. The table must contain latitude and longitude columns (auto-detected from common names such as lat/lon, latitude/longitude, y/x) unless you set lat and lon explicitly." },
                    "lat": { "type": "string", "default": "", "description": "Latitude column name or 1-based column index. Leave blank to auto-detect from common headers such as lat, latitude, y, northing, lat_dd." },
                    "lon": { "type": "string", "default": "", "description": "Longitude column name or 1-based column index. Leave blank to auto-detect from common headers such as lon, lng, longitude, x, easting, lon_dd." },
                    "elevation": { "type": "string", "default": "", "description": "Optional elevation/altitude column name or 1-based index. Leave blank to auto-detect common altitude headers, or set '-' to keep elevation as a normal property instead of a third coordinate." },
                    "delimiter": { "type": "string", "enum": ["auto", "comma", "semicolon", "tab", "pipe"], "default": "auto", "description": "Input delimiter for text tables. 'auto' sniffs the header row; use comma, semicolon, tab, or pipe to force a delimiter. JSON input ignores this." },
                    "shape": { "type": "string", "enum": ["points", "line", "polygon"], "default": "points", "description": "GeoJSON shape to create. 'points' returns a FeatureCollection of Point features; 'line' joins rows in order into one LineString feature; 'polygon' joins rows into one closed Polygon ring." },
                    "types": { "type": "string", "enum": ["infer", "string"], "default": "infer", "description": "How non-coordinate properties are typed. 'infer' converts plain numbers, booleans and blanks to JSON values while preserving identifiers like leading-zero ZIP codes; 'string' keeps CSV cells as strings." },
                    "precision": { "type": "integer", "minimum": 0, "maximum": 15, "default": 0, "description": "Decimal places for coordinates and elevation. 0 keeps full parsed precision; 1-15 rounds coordinates after validation." },
                    "invalid": { "type": "string", "enum": ["skip", "error", "null"], "default": "skip", "description": "How to handle rows with missing, non-numeric, or out-of-range coordinates. 'skip' omits them, 'error' stops with the first row error, and 'null' keeps a Feature with null geometry and the row properties." },
                    "bbox": { "type": "boolean", "default": false, "description": "Add a GeoJSON bbox array spanning all valid coordinates. Default false." },
                    "pretty": { "type": "boolean", "default": true, "description": "Pretty-print GeoJSON with two-space indentation. Disable for compact one-line JSON." }
                },
                "required": ["input"],
                "additionalProperties": false
            }"#,
        ).unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }

    #[test]
    fn args_defaults_match_core_defaults() {
        let a: Args = serde_json::from_str(r#"{"input":"lat,lon\n40,-105\n"}"#).unwrap();
        let d = Options::default();
        assert_eq!(a.lat, d.lat);
        assert_eq!(a.lon, d.lon);
        assert_eq!(a.elevation, d.elevation);
        assert_eq!(a.delimiter, d.delimiter);
        assert_eq!(a.shape, d.shape);
        assert_eq!(a.types, d.types);
        assert_eq!(a.precision, d.precision);
        assert_eq!(a.invalid, d.invalid);
        assert_eq!(a.bbox, d.bbox);
        assert_eq!(a.pretty, d.pretty);
    }
}
