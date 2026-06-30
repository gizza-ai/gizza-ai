//! gizza-ai/subtitle-merge — combine two or more SubRip (.srt) / WebVTT (.vtt)
//! subtitle files into one, sorted by start time and renumbered, with an
//! optional cumulative time-shift.
//!
//! Thin chat-skill wrapper around `gizza-ai-subtitle-merge-core`. The chat
//! schema is single-sourced from `descriptor()` (shared across chat + CLI); the
//! handler delegates to `block_utils::run_skill`. Pure compute — no host calls.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_subtitle_merge_core::{merge, OutFormat};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    subtitles: String,
    #[serde(default)]
    offset_ms: i64,
    #[serde(default)]
    format: String,
}

/// Single source for the chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("subtitles")
                .required()
                .describe("Two or more subtitle files (SubRip .srt or WebVTT .vtt) concatenated into one block, each file separated by a line containing only '===' (three or more equals signs). Each file keeps its own cue numbering/timing lines and dialogue."),
        )
        .param(
            Param::integer("offset_ms")
                .default(0)
                .describe("Cumulative time-shift in milliseconds applied to each file AFTER the first: file 2 is shifted by offset_ms, file 3 by 2×offset_ms, and so on. Use a positive value to append sequentially-split parts (e.g. CD1/CD2) onto one timeline; leave 0 to overlay tracks that already share a timeline. Default 0."),
        )
        .param(
            Param::enumv("format", ["auto", "srt", "vtt"])
                .default("auto")
                .describe("Output format: 'auto' (default) matches the first file's format, 'srt' forces SubRip, 'vtt' forces WebVTT."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/subtitle-merge",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Merge multiple SRT/VTT subtitle files into one.",
    skill(
        description = "Merge (combine) two or more SubRip (.srt) and/or WebVTT (.vtt) subtitle files into a single file. Provide the files as `subtitles`, concatenated and separated by a line containing only '===' . Every cue from every file is collected, sorted by start time (ties keep input order, so a second language track overlays cleanly), renumbered from 1, and re-emitted. `offset_ms` shifts each file after the first by a cumulative multiple of that millisecond amount — use it to line up sequentially-split parts (CD1/CD2) on one timeline, or leave 0 to overlay. `format` is 'auto' (match the first file), 'srt', or 'vtt'. Cue settings/headers are normalized; timestamps below zero clamp to 00:00:00. Runs locally.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "subtitle-merge", |a: Args| {
            let out = OutFormat::parse(&a.format).map_err(SkillError::InvalidArgs)?;
            merge(&a.subtitles, a.offset_ms, out).map_err(SkillError::InvalidArgs)
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
    /// schema, so any future change to the LLM-facing API is intentional.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "subtitles": { "type": "string", "description": "Two or more subtitle files (SubRip .srt or WebVTT .vtt) concatenated into one block, each file separated by a line containing only '===' (three or more equals signs). Each file keeps its own cue numbering/timing lines and dialogue." },
                    "offset_ms": { "type": "integer", "default": 0, "description": "Cumulative time-shift in milliseconds applied to each file AFTER the first: file 2 is shifted by offset_ms, file 3 by 2×offset_ms, and so on. Use a positive value to append sequentially-split parts (e.g. CD1/CD2) onto one timeline; leave 0 to overlay tracks that already share a timeline. Default 0." },
                    "format": { "type": "string", "enum": ["auto", "srt", "vtt"], "default": "auto", "description": "Output format: 'auto' (default) matches the first file's format, 'srt' forces SubRip, 'vtt' forces WebVTT." }
                },
                "required": ["subtitles"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
