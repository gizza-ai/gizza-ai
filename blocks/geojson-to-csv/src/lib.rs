//! gizza-ai/geojson-to-csv — chat skill block on the shared tool abstraction.
//! Flatten GeoJSON features into a CSV table (one row per feature). The chat
//! schema is single-sourced from descriptor() (which also drives the CLI and,
//! via manifest.json, the page form); handle() delegates to block_utils::run_skill.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_geojson_to_csv_core::convert_str;
use serde::Deserialize;
use wafer_sdk::*;

fn default_geometry() -> String {
    "wkt".into()
}
fn default_nested() -> String {
    "json".into()
}
fn default_delimiter() -> String {
    "comma".into()
}
fn default_header() -> bool {
    true
}

#[derive(Deserialize)]
struct Args {
    geojson: String,
    #[serde(default = "default_geometry")]
    geometry: String,
    #[serde(default = "default_nested")]
    nested: String,
    #[serde(default = "default_delimiter")]
    delimiter: String,
    #[serde(default = "default_header")]
    header: bool,
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("geojson").required().describe(
            "GeoJSON to convert. Accepts a FeatureCollection, a single Feature, a bare geometry \
             (Point, MultiPoint, LineString, MultiLineString, Polygon, MultiPolygon, \
             GeometryCollection), or a top-level array of any of those. Coordinates are \
             [longitude, latitude]. Each row is one feature; every properties key becomes a \
             column (union across all features, in first-seen order).",
        ))
        .param(
            Param::enumv("geometry", ["wkt", "lonlat", "both", "none"])
                .default("wkt")
                .describe(
                    "How each feature's geometry is written: 'wkt' (default) a single `geometry` \
                     column of 2-D Well-Known Text (POINT/LINESTRING/POLYGON…); 'lonlat' \
                     `longitude`+`latitude` columns from the first coordinate; 'both' the WKT and \
                     lon/lat columns; 'none' drops geometry (properties only).",
                ),
        )
        .param(
            Param::enumv("nested", ["json", "flatten"])
                .default("json")
                .describe(
                    "How nested property objects/arrays are represented: 'json' (default) keeps \
                     them in one column as compact JSON text; 'flatten' expands them into \
                     dot-notated leaf columns (`address.city`, `tags.0`).",
                ),
        )
        .param(
            Param::enumv("delimiter", ["comma", "semicolon", "tab", "pipe"])
                .default("comma")
                .describe(
                    "Output field separator: 'comma' (default), 'semicolon', 'tab' or 'pipe'. \
                     Fields are RFC-4180 escaped (quoted when they contain the delimiter, a quote \
                     or a newline).",
                ),
        )
        .param(
            Param::boolean("header")
                .default(true)
                .describe("Emit a header row of column names. Default true."),
        )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/geojson-to-csv",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Flatten GeoJSON features into a CSV table",
    skill(
        description = "Flatten GeoJSON features into a CSV table, one row per feature. Pass `geojson` as a FeatureCollection, a single Feature, a bare geometry (Point, LineString, Polygon and their Multi/Collection variants), or a top-level array of any of those; coordinates are [longitude, latitude]. Every `properties` key becomes a column (union across all features, in first-seen order; missing values are blank). Set `geometry` to 'wkt' (default, a `geometry` column of 2-D Well-Known Text), 'lonlat' (`longitude`+`latitude` from the first coordinate), 'both', or 'none'. Use `nested`='flatten' to expand nested objects/arrays into dot-notated columns (`address.city`, `tags.0`) instead of JSON text. `delimiter` picks comma/semicolon/tab/pipe and `header`=false drops the header row. Output is RFC-4180 escaped. Runs locally on the device.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "geojson-to-csv", |a: Args| {
            convert_str(&a.geojson, &a.geometry, &a.nested, &a.delimiter, a.header)
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
            r##"{
                "type": "object",
                "properties": {
                    "geojson": { "type": "string", "description": "GeoJSON to convert. Accepts a FeatureCollection, a single Feature, a bare geometry (Point, MultiPoint, LineString, MultiLineString, Polygon, MultiPolygon, GeometryCollection), or a top-level array of any of those. Coordinates are [longitude, latitude]. Each row is one feature; every properties key becomes a column (union across all features, in first-seen order)." },
                    "geometry": { "type": "string", "enum": ["wkt", "lonlat", "both", "none"], "default": "wkt", "description": "How each feature's geometry is written: 'wkt' (default) a single `geometry` column of 2-D Well-Known Text (POINT/LINESTRING/POLYGON…); 'lonlat' `longitude`+`latitude` columns from the first coordinate; 'both' the WKT and lon/lat columns; 'none' drops geometry (properties only)." },
                    "nested": { "type": "string", "enum": ["json", "flatten"], "default": "json", "description": "How nested property objects/arrays are represented: 'json' (default) keeps them in one column as compact JSON text; 'flatten' expands them into dot-notated leaf columns (`address.city`, `tags.0`)." },
                    "delimiter": { "type": "string", "enum": ["comma", "semicolon", "tab", "pipe"], "default": "comma", "description": "Output field separator: 'comma' (default), 'semicolon', 'tab' or 'pipe'. Fields are RFC-4180 escaped (quoted when they contain the delimiter, a quote or a newline)." },
                    "header": { "type": "boolean", "default": true, "description": "Emit a header row of column names. Default true." }
                },
                "required": ["geojson"],
                "additionalProperties": false
            }"##,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
