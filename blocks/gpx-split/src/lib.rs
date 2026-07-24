//! gizza-ai/gpx-split — chat skill block on the shared tool abstraction.
//! Splits one GPX track into multiple segments by distance covered, elapsed
//! time, or detected stop/pause gaps, returning per-segment stats plus a new
//! multi-track GPX (or just the summary). The chat schema is single-sourced from
//! descriptor() (which also drives the CLI); handle() delegates to run_skill.
//! Pure → runs on all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_gpx_split_core::{split_json, Config, Mode, Output, Unit};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    gpx: String,
    #[serde(default = "default_mode")]
    mode: String,
    #[serde(default = "default_distance")]
    distance: f64,
    #[serde(default = "default_unit")]
    unit: String,
    #[serde(default = "default_time_min")]
    time_min: f64,
    #[serde(default = "default_stop_gap_s")]
    stop_gap_s: f64,
    #[serde(default = "default_output")]
    output: String,
}

fn default_mode() -> String {
    "distance".to_string()
}
fn default_distance() -> f64 {
    5.0
}
fn default_unit() -> String {
    "km".to_string()
}
fn default_time_min() -> f64 {
    30.0
}
fn default_stop_gap_s() -> f64 {
    120.0
}
fn default_output() -> String {
    "gpx".to_string()
}

/// Single source for the chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("gpx")
                .required()
                .describe("The full GPX file contents (XML). Track points may include <ele> elevation and <time> timestamps; timestamps are required for the time and stops split modes."),
        )
        .param(
            Param::enumv("mode", ["distance", "time", "stops"])
                .default("distance")
                .describe("How to cut the track: distance (every N km/mi covered), time (every N minutes of elapsed time), or stops (start a new segment at each pause, i.e. a time gap between points). Default distance."),
        )
        .param(
            Param::number("distance")
                .default(5.0)
                .min(0.01)
                .describe("Segment length for distance mode, in the chosen unit. Default 5."),
        )
        .param(
            Param::enumv("unit", ["km", "mi"])
                .default("km")
                .describe("Distance unit for the distance-mode threshold: km or mi. Default km."),
        )
        .param(
            Param::number("time_min")
                .default(30.0)
                .min(0.01)
                .describe("Segment length for time mode, in minutes. Default 30."),
        )
        .param(
            Param::number("stop_gap_s")
                .default(120.0)
                .min(1.0)
                .describe("For stops mode: the minimum time gap between two consecutive points (in seconds) that starts a new segment. Default 120."),
        )
        .param(
            Param::enumv("output", ["gpx", "summary"])
                .default("gpx")
                .describe("What to return: gpx (a GPX document with one named track per segment) or summary (a per-segment table of distance, duration and point count). Default gpx."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

fn build_config(a: &Args) -> Result<Config, String> {
    Ok(Config {
        mode: Mode::parse(&a.mode)?,
        distance: a.distance,
        unit: Unit::parse(&a.unit)?,
        time_min: a.time_min,
        stop_gap_s: a.stop_gap_s,
        output: Output::parse(&a.output)?,
    })
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/gpx-split",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Split a GPX track into segments by distance, time, or stops",
    skill(
        description = "Split one GPX track into multiple segments and return a new GPX (one named track per segment) plus per-segment stats. Three modes: distance (a new segment every N km or miles covered), time (a new segment every N minutes of elapsed time), or stops (a new segment wherever the recording pauses — the time gap between two points exceeds a threshold). Reads <trkpt>, <rtept>, and <wpt> points; preserves <ele> elevation and <time> timestamps. Distances are great-circle (haversine). Runs locally.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "gpx-split", |a: Args| {
            let cfg = build_config(&a).map_err(SkillError::InvalidArgs)?;
            split_json(&a.gpx, &cfg).map_err(SkillError::InvalidArgs)
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
                    "gpx": { "type": "string", "description": "The full GPX file contents (XML). Track points may include <ele> elevation and <time> timestamps; timestamps are required for the time and stops split modes." },
                    "mode": { "type": "string", "enum": ["distance", "time", "stops"], "default": "distance", "description": "How to cut the track: distance (every N km/mi covered), time (every N minutes of elapsed time), or stops (start a new segment at each pause, i.e. a time gap between points). Default distance." },
                    "distance": { "type": "number", "minimum": 0.01, "default": 5.0, "description": "Segment length for distance mode, in the chosen unit. Default 5." },
                    "unit": { "type": "string", "enum": ["km", "mi"], "default": "km", "description": "Distance unit for the distance-mode threshold: km or mi. Default km." },
                    "time_min": { "type": "number", "minimum": 0.01, "default": 30.0, "description": "Segment length for time mode, in minutes. Default 30." },
                    "stop_gap_s": { "type": "number", "minimum": 1, "default": 120.0, "description": "For stops mode: the minimum time gap between two consecutive points (in seconds) that starts a new segment. Default 120." },
                    "output": { "type": "string", "enum": ["gpx", "summary"], "default": "gpx", "description": "What to return: gpx (a GPX document with one named track per segment) or summary (a per-segment table of distance, duration and point count). Default gpx." }
                },
                "required": ["gpx"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
