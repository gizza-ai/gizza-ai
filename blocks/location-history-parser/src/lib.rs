//! gizza-ai/location-history-parser — chat skill block on the shared tool abstraction.
//! The chat schema is single-sourced from descriptor() (which also drives the
//! CLI); handle() delegates to block_utils::run_skill. Parses a Google Takeout /
//! Dawarich location-history JSON export into a per-day summary of places
//! visited and distance traveled.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

fn default_output() -> String {
    "text".into()
}
fn default_unit() -> String {
    "km".into()
}
fn default_place_radius() -> f64 {
    150.0
}
fn default_min_stay() -> f64 {
    10.0
}

#[derive(Deserialize)]
struct Args {
    input: String,
    #[serde(default = "default_output")]
    output: String,
    #[serde(default = "default_unit")]
    unit: String,
    #[serde(default)]
    utc_offset: f64,
    #[serde(default = "default_place_radius")]
    place_radius: f64,
    #[serde(default = "default_min_stay")]
    min_stay: f64,
}

/// Single source for the chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("input").required().multiline().describe("Location-history JSON export to summarize. Accepts Google Takeout classic Records (a \"locations\" array), Semantic Location History (\"timelineObjects\"), the newer on-device Timeline (\"semanticSegments\"), a GeoJSON Point FeatureCollection, or a plain array of {latitude, longitude, timestamp} points (Dawarich / generic)."))
        .param(Param::enumv("output", ["text", "csv", "json"]).default("text").describe("Output format: text (a per-day summary plus totals), csv (date,places,distance rows), or json (structured days with place names and totals). Default text."))
        .param(Param::enumv("unit", ["km", "mi"]).default("km").describe("Distance unit for the report: km (kilometers) or mi (miles). Default km."))
        .param(Param::number("utc_offset").min(-14.0).max(14.0).default(0.0).describe("Hours to add to UTC timestamps before bucketing into calendar days, so days match your local time (e.g. -5 for US Eastern, 5.5 for India). Default 0 (UTC)."))
        .param(Param::number("place_radius").min(1.0).max(100000.0).default(150.0).describe("Raw GPS streams only: proximity radius in meters for stop detection. Consecutive points within this radius of a cluster anchor form one visited place. Ignored for exports that already carry named places. Default 150."))
        .param(Param::number("min_stay").min(0.0).max(10000.0).default(10.0).describe("Raw GPS streams only: minimum dwell in minutes for a point cluster to count as a visited place. Ignored for exports that already carry named places. Default 10."))
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/location-history-parser",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Summarize a Google Takeout or Dawarich location-history JSON export into per-day places visited and distance traveled.",
    skill(
        description = "Parse a location-history JSON export into a per-day summary of places visited and distance traveled. Auto-detects Google Takeout classic Records (locations[]), Semantic Location History (timelineObjects[]), the newer on-device Timeline (semanticSegments[]), GeoJSON Point FeatureCollections, and plain {latitude,longitude,timestamp} point arrays (Dawarich / generic). Named exports use their own visited places and activity distances; raw GPS streams get stop detection (place_radius meters + min_stay minutes) and great-circle (haversine) distance. Choose km or mi, a UTC offset so days match your local calendar, and text, CSV, or JSON output. Runs entirely locally; nothing is uploaded.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "location-history-parser", |a: Args| {
            gizza_ai_location_history_parser_core::run(
                &a.input,
                &a.output,
                &a.unit,
                a.utc_offset,
                a.place_radius,
                a.min_stay,
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
        let authored: serde_json::Value = serde_json::from_str(r#"{
          "type":"object","properties":{
            "input":{"type":"string","description":"Location-history JSON export to summarize. Accepts Google Takeout classic Records (a \"locations\" array), Semantic Location History (\"timelineObjects\"), the newer on-device Timeline (\"semanticSegments\"), a GeoJSON Point FeatureCollection, or a plain array of {latitude, longitude, timestamp} points (Dawarich / generic)."},
            "output":{"type":"string","enum":["text","csv","json"],"default":"text","description":"Output format: text (a per-day summary plus totals), csv (date,places,distance rows), or json (structured days with place names and totals). Default text."},
            "unit":{"type":"string","enum":["km","mi"],"default":"km","description":"Distance unit for the report: km (kilometers) or mi (miles). Default km."},
            "utc_offset":{"type":"number","minimum":-14,"maximum":14,"default":0.0,"description":"Hours to add to UTC timestamps before bucketing into calendar days, so days match your local time (e.g. -5 for US Eastern, 5.5 for India). Default 0 (UTC)."},
            "place_radius":{"type":"number","minimum":1,"maximum":100000,"default":150.0,"description":"Raw GPS streams only: proximity radius in meters for stop detection. Consecutive points within this radius of a cluster anchor form one visited place. Ignored for exports that already carry named places. Default 150."},
            "min_stay":{"type":"number","minimum":0,"maximum":10000,"default":10.0,"description":"Raw GPS streams only: minimum dwell in minutes for a point cluster to count as a visited place. Ignored for exports that already carry named places. Default 10."}
          },"required":["input"],"additionalProperties":false
        }"#).unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
