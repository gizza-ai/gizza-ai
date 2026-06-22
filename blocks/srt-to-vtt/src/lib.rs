//! gizza-ai/srt-to-vtt — convert subtitles between SubRip (.srt) and WebVTT (.vtt).
//!
//! Thin chat-skill wrapper around `gizza-ai-srt-to-vtt-core`. The chat schema is
//! single-sourced from `descriptor()` (shared shape across chat + CLI); the
//! handler delegates to `block_utils::run_skill`. Pure compute — no host calls.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_srt_to_vtt_core::{auto_convert, convert, Direction};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    subtitles: String,
    /// "auto" (default), "srt-to-vtt", or "vtt-to-srt".
    #[serde(default)]
    direction: String,
}

/// Run the conversion described by `direction` on `subtitles`.
fn run_convert(subtitles: &str, direction: &str) -> Result<String, String> {
    match direction {
        "" | "auto" => auto_convert(subtitles),
        "srt-to-vtt" => convert(subtitles, Direction::SrtToVtt),
        "vtt-to-srt" => convert(subtitles, Direction::VttToSrt),
        other => Err(format!(
            "invalid direction {other:?}: expected \"auto\", \"srt-to-vtt\", or \"vtt-to-srt\""
        )),
    }
}

/// Single-source param descriptor → chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("subtitles")
                .required()
                .describe("The subtitle text to convert: a full SubRip (.srt) or WebVTT (.vtt) file, including the cue numbers, the timing lines, and the dialogue."),
        )
        .param(
            Param::enumv("direction", ["auto", "srt-to-vtt", "vtt-to-srt"])
                .default("auto")
                .describe("Which way to convert. 'auto' (default) detects the input format and converts to the other; 'srt-to-vtt' forces SubRip → WebVTT; 'vtt-to-srt' forces WebVTT → SubRip."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/srt-to-vtt",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Convert subtitles between SubRip (.srt) and WebVTT (.vtt).",
    skill(
        description = "Convert subtitles between SubRip (.srt) and WebVTT (.vtt) formats. Pass the full subtitle text as `subtitles`. With `direction` = 'auto' (default) the input format is detected and converted to the other; 'srt-to-vtt' forces SubRip → WebVTT and 'vtt-to-srt' forces WebVTT → SubRip. The conversion rewrites the timing-line decimal separator (SRT comma `00:00:01,000` ↔ WebVTT period `00:00:01.000`), adds or strips the leading WEBVTT signature/header, and expands WebVTT's short MM:SS.mmm timestamps to HH:MM:SS,mmm. Cue numbers and dialogue text are kept verbatim; WebVTT cue settings (e.g. line:90%) are dropped when producing SRT.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "srt-to-vtt", |a: Args| {
            run_convert(&a.subtitles, &a.direction).map_err(SkillError::InvalidArgs)
        }) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SRT: &str = "1\n00:00:01,000 --> 00:00:04,000\nHi\n";
    const VTT: &str = "WEBVTT\n\n1\n00:00:01.000 --> 00:00:04.000\nHi\n";

    #[test]
    fn auto_srt_to_vtt() {
        let out = run_convert(SRT, "auto").unwrap();
        assert!(out.starts_with("WEBVTT"), "{out:?}");
    }

    #[test]
    fn auto_vtt_to_srt() {
        let out = run_convert(VTT, "").unwrap();
        assert!(!out.contains("WEBVTT"), "{out:?}");
        assert!(out.contains("00:00:01,000 --> 00:00:04,000"), "{out}");
    }

    #[test]
    fn forced_directions() {
        assert!(run_convert(SRT, "srt-to-vtt").unwrap().starts_with("WEBVTT"));
        assert!(run_convert(VTT, "vtt-to-srt").unwrap().contains("00:00:01,000"));
    }

    #[test]
    fn rejects_bad_direction() {
        assert!(run_convert(SRT, "sideways").is_err());
    }

    /// Drift guard: the descriptor-derived chat schema must match this authored
    /// schema, so any future change to the LLM-facing API is intentional.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "subtitles": { "type": "string", "description": "The subtitle text to convert: a full SubRip (.srt) or WebVTT (.vtt) file, including the cue numbers, the timing lines, and the dialogue." },
                    "direction": { "type": "string", "enum": ["auto", "srt-to-vtt", "vtt-to-srt"], "default": "auto", "description": "Which way to convert. 'auto' (default) detects the input format and converts to the other; 'srt-to-vtt' forces SubRip → WebVTT; 'vtt-to-srt' forces WebVTT → SubRip." }
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
