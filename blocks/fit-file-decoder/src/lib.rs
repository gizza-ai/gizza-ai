//! gizza-ai/fit-file-decoder — chat skill block on the shared tool abstraction.
//! Decodes a binary Garmin/ANT FIT activity file (base64) into a readable
//! summary, a CSV table, or a GPX track. The chat schema is single-sourced from
//! descriptor() (which also drives the CLI); handle() delegates to run_skill.
//! Pure → runs on all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_fit_file_decoder_core::decode_str;
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    data: String,
    #[serde(default = "default_format")]
    format: String,
}

fn default_format() -> String {
    "summary".to_string()
}

/// Single source for the chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("data")
                .required()
                .describe("The .fit file contents encoded as base64 (standard or URL-safe alphabet, whitespace tolerated). Decode the binary FIT activity file to base64 first; the raw bytes must start with the 12-byte FIT header carrying the \".FIT\" signature. Maximum 8 MiB decoded."),
        )
        .param(
            Param::enumv("format", ["summary", "csv", "gpx"])
                .default("summary")
                .describe("Output shape: summary (header, record count, time range, GPS bounding box and the session totals/averages), csv (one row per record with timestamp, position, altitude, distance, speed, heart rate, cadence and power), or gpx (a GPX 1.1 track with elevation, time and heart-rate/cadence/power extensions). Default summary."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/fit-file-decoder",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Decode a base64 FIT activity file to a summary, CSV, or GPX",
    skill(
        description = "Decode a binary Garmin/ANT FIT activity file (supplied as base64) into readable output. Choose summary for the protocol/profile version, record count, time range, GPS bounding box and the session totals and averages (sport, distance, time, calories, speed, heart rate, power, ascent/descent); csv for one row per record with timestamp, latitude, longitude, altitude, distance, speed, heart rate, cadence and power; or gpx for a GPX 1.1 track with <ele>, <time> and heart-rate/cadence/power extensions. Handles little- and big-endian FIT, compressed timestamps, and developer fields (skipped). Positions are converted from semicircles to degrees; unknown messages are ignored, not errored. Runs locally.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "fit-file-decoder", |a: Args| {
            decode_str(&a.data, &a.format).map_err(SkillError::InvalidArgs)
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
                    "data": { "type": "string", "description": "The .fit file contents encoded as base64 (standard or URL-safe alphabet, whitespace tolerated). Decode the binary FIT activity file to base64 first; the raw bytes must start with the 12-byte FIT header carrying the \".FIT\" signature. Maximum 8 MiB decoded." },
                    "format": { "type": "string", "enum": ["summary", "csv", "gpx"], "default": "summary", "description": "Output shape: summary (header, record count, time range, GPS bounding box and the session totals/averages), csv (one row per record with timestamp, position, altitude, distance, speed, heart rate, cadence and power), or gpx (a GPX 1.1 track with elevation, time and heart-rate/cadence/power extensions). Default summary." }
                },
                "required": ["data"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
