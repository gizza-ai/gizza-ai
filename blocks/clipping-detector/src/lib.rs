//! gizza-ai/clipping-detector — chat skill block on the shared tool abstraction.
//! Decodes a short uncompressed WAV clip (base64 or hex bytes) and scans the
//! decoded PCM for full-scale (clipped) samples, grouping consecutive clipped
//! sample-frames into runs ("regions") and reporting WHERE the distortion is
//! with start/end timestamps. The chat schema is single-sourced from
//! `descriptor()` (which also drives the CLI); `handle()` delegates to
//! `block_utils::run_skill`. No host calls — runs entirely inside the WASM
//! sandbox.
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
    #[serde(default = "default_threshold")]
    threshold: f64,
    #[serde(default = "default_min_run")]
    min_run: u32,
    #[serde(default)]
    gap: u32,
    #[serde(default = "default_top_regions")]
    top_regions: u32,
}

fn default_threshold() -> f64 {
    0.99
}
fn default_min_run() -> u32 {
    1
}
fn default_top_regions() -> u32 {
    5
}

/// Single source for the chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("input")
                .required()
                .describe("Uncompressed WAV audio bytes encoded as base64 (default) or hex. Only RIFF/WAVE PCM 8/16/24/32-bit integer or 32/64-bit IEEE float is decoded; compressed MP3/AAC/FLAC/Ogg/A-law/mu-law is rejected with a clear message."),
        )
        .param(
            Param::enumv("input_format", ["base64", "hex"])
                .default("base64")
                .describe("Encoding of the pasted WAV bytes: 'base64' (default) or 'hex'. Hex may include whitespace, ':' or '-' separators."),
        )
        .param(
            Param::enumv("output", ["report", "json"])
                .default("report")
                .describe("Output format. 'report' (default) is a human-readable summary (format, duration, peak, clipped counts, and the worst regions with start-end timestamps). 'json' emits a structured object {format, sample_rate, channels, peak, clipped_samples, clipped_frames, longest_run_frames, region_count, regions:[{start_frame, end_frame, start_time, end_time, peak_dbfs}]}."),
        )
        .param(
            Param::number("threshold")
                .min(0.5)
                .max(1.0)
                .default(0.99)
                .describe("A sample counts as clipped when its magnitude reaches this fraction of full scale (0.5-1.0). Default 0.99 catches true 0 dBFS peaks with a small guard band; 1.0 flags only exact full-scale samples."),
        )
        .param(
            Param::integer("min_run")
                .min(1.0)
                .max(1000.0)
                .default(1)
                .describe("Minimum number of consecutive clipped sample-frames for a run to count as a clipped region (1-1000, default 1). Audacity's classic rule is 3 consecutive full-scale samples."),
        )
        .param(
            Param::integer("gap")
                .min(0.0)
                .max(1000.0)
                .default(0)
                .describe("Bridge runs separated by at most this many unclipped frames into one region so a single distorted burst isn't fragmented (0-1000, default 0 = no bridging)."),
        )
        .param(
            Param::integer("top_regions")
                .min(1.0)
                .max(100.0)
                .default(5)
                .describe("How many of the worst regions to list, ranked by length then peak (1-100, default 5)."),
        )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/clipping-detector",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Scan an uncompressed WAV clip for clipped (full-scale) samples and report the worst distorted regions with timestamps.",
    skill(
        description = "Detect digital clipping in a short uncompressed WAV clip. Paste the WAV bytes as base64 (default) or hex; it decodes RIFF/WAVE PCM 8/16/24/32-bit integer or 32/64-bit IEEE float (compressed MP3/AAC/FLAC/Ogg/A-law/mu-law is rejected with a clear message) and scans for samples at or above a full-scale threshold. It reports how many samples and sample-frames clipped, the overall peak in dBFS, the longest run of consecutive clipped frames, and the worst regions with their start-end timestamps so you can see WHERE the distortion is. threshold (0.5-1.0, default 0.99) sets what counts as full scale; min_run (default 1) is the minimum consecutive clipped frames for a region (Audacity's rule is 3); gap (default 0) bridges runs split by a few clean frames; top_regions (default 5) caps how many regions are listed. output is 'report' (human-readable, default) or 'json'. Runs entirely in the sandbox; nothing is uploaded.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "clipping-detector", |a: Args| {
            gizza_ai_clipping_detector_core::run(
                &a.input,
                &a.input_format,
                &a.output,
                a.threshold,
                a.min_run,
                a.gap,
                a.top_regions,
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

    /// Drift guard: the descriptor-derived chat schema must match this authored
    /// schema, so any future change to the LLM-facing API is intentional and
    /// reviewed. Authored 2026-07-27 for the initial clipping-detector release.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "input": { "type": "string", "description": "Uncompressed WAV audio bytes encoded as base64 (default) or hex. Only RIFF/WAVE PCM 8/16/24/32-bit integer or 32/64-bit IEEE float is decoded; compressed MP3/AAC/FLAC/Ogg/A-law/mu-law is rejected with a clear message." },
                    "input_format": { "type": "string", "enum": ["base64", "hex"], "default": "base64", "description": "Encoding of the pasted WAV bytes: 'base64' (default) or 'hex'. Hex may include whitespace, ':' or '-' separators." },
                    "output": { "type": "string", "enum": ["report", "json"], "default": "report", "description": "Output format. 'report' (default) is a human-readable summary (format, duration, peak, clipped counts, and the worst regions with start-end timestamps). 'json' emits a structured object {format, sample_rate, channels, peak, clipped_samples, clipped_frames, longest_run_frames, region_count, regions:[{start_frame, end_frame, start_time, end_time, peak_dbfs}]}." },
                    "threshold": { "type": "number", "minimum": 0.5, "maximum": 1, "default": 0.99, "description": "A sample counts as clipped when its magnitude reaches this fraction of full scale (0.5-1.0). Default 0.99 catches true 0 dBFS peaks with a small guard band; 1.0 flags only exact full-scale samples." },
                    "min_run": { "type": "integer", "minimum": 1, "maximum": 1000, "default": 1, "description": "Minimum number of consecutive clipped sample-frames for a run to count as a clipped region (1-1000, default 1). Audacity's classic rule is 3 consecutive full-scale samples." },
                    "gap": { "type": "integer", "minimum": 0, "maximum": 1000, "default": 0, "description": "Bridge runs separated by at most this many unclipped frames into one region so a single distorted burst isn't fragmented (0-1000, default 0 = no bridging)." },
                    "top_regions": { "type": "integer", "minimum": 1, "maximum": 100, "default": 5, "description": "How many of the worst regions to list, ranked by length then peak (1-100, default 5)." }
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
