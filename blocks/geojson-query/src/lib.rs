//! gizza-ai/geojson-query — chat skill block on the shared tool abstraction.
//! The chat schema is single-sourced from descriptor() (which also drives the
//! CLI); handle() delegates to block_utils::run_skill. The new-tool skill edits
//! descriptor()'s params + core::run to the tool's real inputs/logic.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    input: String,
    #[serde(default = "default_query")]
    query: String,
    #[serde(default)]
    bbox: String,
    #[serde(default = "default_format")]
    format: String,
    #[serde(default)]
    indent: i64,
    #[serde(default)]
    include_bbox: bool,
}

fn default_query() -> String {
    "SELECT *".into()
}
fn default_format() -> String {
    "geojson".into()
}

/// Single source for the chat schema (and CLI). Edit the params to match the
/// tool's real inputs — e.g. `.param(Param::enumv("mode", ["a","b"]).default("a"))`,
/// `.param(Param::integer("n").min(1.0))`. Use Input::Image/Video/Document/File
/// for tools that take a url/ref media input (see image-resize / web-fetch).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("input").required().describe("GeoJSON FeatureCollection, Feature, bare geometry, or array of Feature objects to query. Coordinates are interpreted as WGS84 lon/lat."))
        .param(Param::string("query").default("SELECT *").describe("SQL-like query. Supports SELECT fields or aggregates, optional WHERE with property comparisons and bbox()/within()/contains()/near(), GROUP BY, ORDER BY, LIMIT, and OFFSET."))
        .param(Param::string("bbox").default("").describe("Optional bounding-box prefilter written as minLon,minLat,maxLon,maxLat. It is ANDed with any WHERE clause and uses intersects semantics."))
        .param(Param::enumv("format", ["geojson", "json", "csv"]).default("geojson").describe("Output format. geojson returns a FeatureCollection for non-aggregate queries; json returns property or aggregate rows; csv returns those rows as comma-separated text."))
        .param(Param::integer("indent").default(0).min(0.0).max(8.0).describe("JSON indentation spaces from 0 to 8. Use 0 for compact GeoJSON/JSON."))
        .param(Param::boolean("include_bbox").default(false).describe("When true, include a computed bbox member on GeoJSON FeatureCollection output."))
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

fn run_args(a: Args) -> Result<String, String> {
    gizza_ai_geojson_query_core::query(
        &a.input,
        &a.query,
        &a.bbox,
        &a.format,
        a.indent,
        a.include_bbox,
    )
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/geojson-query",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Filter and aggregate GeoJSON features with a SQL-like spatial query.",
    skill(
        description = "Query a pasted GeoJSON FeatureCollection, Feature, bare geometry, or feature array with a deterministic SQL-like language. Supports property filters, AND/OR/NOT, IN, LIKE, bbox/intersects, within, contains(lon,lat), near(lon,lat,km), projection with SELECT, GROUP BY with count/sum/avg/min/max, ORDER BY, LIMIT/OFFSET, bbox prefiltering, and GeoJSON/JSON/CSV output. Coordinates are treated as WGS84 lon/lat and no reprojection is performed.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        // run_skill wraps the returned value in { "result": ... }. For a media
        // tool, use resolve_source + dispatch_ffmpeg + build_media_envelope
        // instead (see blocks/image-resize/src/lib.rs).
        match run_skill(&body, "geojson-query", |a: Args| {
            run_args(a).map_err(SkillError::InvalidArgs)
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
              "type":"object",
              "properties":{
                "input":{"type":"string","description":"GeoJSON FeatureCollection, Feature, bare geometry, or array of Feature objects to query. Coordinates are interpreted as WGS84 lon/lat."},
                "query":{"type":"string","default":"SELECT *","description":"SQL-like query. Supports SELECT fields or aggregates, optional WHERE with property comparisons and bbox()/within()/contains()/near(), GROUP BY, ORDER BY, LIMIT, and OFFSET."},
                "bbox":{"type":"string","default":"","description":"Optional bounding-box prefilter written as minLon,minLat,maxLon,maxLat. It is ANDed with any WHERE clause and uses intersects semantics."},
                "format":{"type":"string","enum":["geojson","json","csv"],"default":"geojson","description":"Output format. geojson returns a FeatureCollection for non-aggregate queries; json returns property or aggregate rows; csv returns those rows as comma-separated text."},
                "indent":{"type":"integer","minimum":0,"maximum":8,"default":0,"description":"JSON indentation spaces from 0 to 8. Use 0 for compact GeoJSON/JSON."},
                "include_bbox":{"type":"boolean","default":false,"description":"When true, include a computed bbox member on GeoJSON FeatureCollection output."}
              },
              "required":["input"],
              "additionalProperties":false
            }"#,
        ).unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
