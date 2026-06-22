//! gizza-ai/gpx-analyzer — parse a GPX track and report distance, elevation
//! gain/loss, duration, and average pace & speed. Chat schema single-sourced
//! from descriptor(); handler delegates to run_skill. Pure → all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_gpx_analyzer_core::analyze;
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    gpx: String,
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None).param(
        Param::string("gpx")
            .required()
            .describe("The full GPX file contents (XML). Track points may include <ele> elevation and <time> timestamps for elevation-gain and pace/speed stats."),
    )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/gpx-analyzer",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Analyze a GPX track: distance, elevation, duration, pace",
    skill(
        description = "Parse a GPX file (GPS track from a watch, phone, or route planner) and report: total distance (km and miles), elevation gain/loss and min/max altitude, duration, average speed (km/h and mph), and average pace (min/km and min/mi). Reads <trkpt>, <rtept>, and <wpt> points; distance is summed great-circle (haversine) between consecutive points. Runs locally.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "gpx-analyzer", |a: Args| {
            analyze(&a.gpx).map_err(SkillError::InvalidArgs)
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
                    "gpx": { "type": "string", "description": "The full GPX file contents (XML). Track points may include <ele> elevation and <time> timestamps for elevation-gain and pace/speed stats." }
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
