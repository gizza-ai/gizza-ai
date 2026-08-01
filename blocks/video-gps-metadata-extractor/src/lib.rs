//! gizza-ai/video-gps-metadata-extractor — chat skill block on the shared tool abstraction.
//! The chat schema is single-sourced from descriptor() (which also drives the
//! CLI); handle() delegates to block_utils::run_skill. No host calls — the whole
//! MP4/MOV box walk runs inside the WASM sandbox.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    input: String,
    #[serde(default)]
    input_format: String,
    #[serde(default)]
    output: String,
}

/// Single source for the chat schema (and CLI). See
/// docs/superpowers/specs/2026-06-19-gizza-shared-tool-abstraction-design.md.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("input")
                .required()
                .describe("The MP4/MOV/QuickTime video file bytes, encoded as a base64 or hex string. Only the metadata region is needed, so a truncated head that reaches the 'moov' box is enough — the full file is not required."),
        )
        .param(
            Param::enumv("input_format", ["base64", "hex"])
                .default("base64")
                .describe("How 'input' is encoded: 'base64' (default; standard or URL-safe, padding optional) or 'hex' (whitespace, ':' and '-' separators are ignored)."),
        )
        .param(
            Param::enumv("output", ["report", "json"])
                .default("report")
                .describe("Output format: 'report' (default, a human-readable summary of each GPS tag) or 'json' (a structured object with count and per-location latitude/longitude/altitude/source/map_url)."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/video-gps-metadata-extractor",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Extract embedded GPS location from MP4/MOV video metadata.",
    skill(
        description = "Extract the GPS location tag embedded in an MP4/MOV/QuickTime video's metadata, entirely in-browser with no ffmpeg. Reads the classic QuickTime '©xyz' user-data atom (moov/udta) that iPhones and many Android phones write, and Apple's newer 'com.apple.quicktime.location.ISO6709' key in the meta/ilst table, parses the ISO 6709 coordinate string into signed decimal latitude/longitude (and altitude when present), and returns an OpenStreetMap link. Pass the file bytes in 'input' as base64 (default) or hex via 'input_format'; only the metadata head that reaches the 'moov' box is required. 'output' is 'report' (default, human-readable) or 'json' (structured). A valid video with no location tag is reported as 'none found', not an error. This is the static location tag, not per-frame GoPro GPMF telemetry.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        // run_skill wraps the returned value in { "result": … } and routes
        // errors through GuestResult::error.
        match run_skill(&body, "video-gps-metadata-extractor", |a: Args| {
            gizza_ai_video_gps_metadata_extractor_core::run(&a.input, &a.input_format, &a.output)
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

    /// Drift guard: the descriptor-derived chat schema must match this authored
    /// schema, so any future change to the LLM-facing API is intentional and
    /// reviewed.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "input": { "type": "string", "description": "The MP4/MOV/QuickTime video file bytes, encoded as a base64 or hex string. Only the metadata region is needed, so a truncated head that reaches the 'moov' box is enough — the full file is not required." },
                    "input_format": { "type": "string", "enum": ["base64", "hex"], "default": "base64", "description": "How 'input' is encoded: 'base64' (default; standard or URL-safe, padding optional) or 'hex' (whitespace, ':' and '-' separators are ignored)." },
                    "output": { "type": "string", "enum": ["report", "json"], "default": "report", "description": "Output format: 'report' (default, a human-readable summary of each GPS tag) or 'json' (a structured object with count and per-location latitude/longitude/altitude/source/map_url)." }
                },
                "required": ["input"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
