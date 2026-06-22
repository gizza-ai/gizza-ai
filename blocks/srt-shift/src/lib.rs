//! gizza-ai/srt-shift — shifts every timestamp in a SubRip (.srt) subtitle file
//! forward or backward by a fixed offset, to re-sync subtitles with their video.
//!
//! Thin chat-skill wrapper around `gizza-ai-srt-shift-core`. The chat schema is
//! single-sourced from `descriptor()` (shared shape across chat + CLI); the
//! handler delegates to `block_utils::run_skill`. Pure compute — no host calls.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    srt: String,
    offset: f64,
    /// "seconds" (default) or "milliseconds" — the unit of `offset`.
    #[serde(default)]
    unit: String,
}

/// Convert the `offset` (in `unit`) to whole milliseconds for the core.
fn offset_to_ms(offset: f64, unit: &str) -> Result<i64, String> {
    if !offset.is_finite() {
        return Err("offset must be a finite number".into());
    }
    let ms = match unit {
        "" | "seconds" => offset * 1000.0,
        "milliseconds" => offset,
        other => {
            return Err(format!(
                "invalid unit {other:?}: expected \"seconds\" or \"milliseconds\""
            ))
        }
    };
    Ok(ms.round() as i64)
}

/// Single-source param descriptor → chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("srt")
                .required()
                .describe("The SubRip (.srt) subtitle text to retime, including the cue numbers, the 'HH:MM:SS,mmm --> HH:MM:SS,mmm' timing lines, and the dialogue."),
        )
        .param(
            Param::number("offset")
                .required()
                .describe("How far to shift every timestamp. Positive = later (delay subtitles), negative = earlier (advance them). Interpreted in `unit`."),
        )
        .param(
            Param::enumv("unit", ["seconds", "milliseconds"])
                .default("seconds")
                .describe("The unit of `offset`: 'seconds' (default, fractions allowed e.g. 2.5) or 'milliseconds'."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/srt-shift",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Shift all SubRip (.srt) subtitle timings by an offset.",
    skill(
        description = "Shift every timestamp in a SubRip (.srt) subtitle file forward or backward by a fixed offset, to re-sync subtitles that run early or late against their video. Pass the full subtitle text as `srt` and the amount as `offset` (positive delays the subtitles, negative advances them). `unit` is 'seconds' (default, fractions allowed e.g. offset=2.5) or 'milliseconds'. Cue numbers and dialogue text are kept verbatim; only the timing lines change. Any timestamp that would go below zero is clamped to 00:00:00,000.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "srt-shift", |a: Args| {
            let ms = offset_to_ms(a.offset, &a.unit).map_err(SkillError::InvalidArgs)?;
            gizza_ai_srt_shift_core::shift(&a.srt, ms).map_err(SkillError::InvalidArgs)
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
    fn offset_seconds_to_ms() {
        assert_eq!(offset_to_ms(2.5, "seconds").unwrap(), 2500);
        assert_eq!(offset_to_ms(2.5, "").unwrap(), 2500);
        assert_eq!(offset_to_ms(-1.2, "seconds").unwrap(), -1200);
    }

    #[test]
    fn offset_milliseconds_passthrough() {
        assert_eq!(offset_to_ms(750.0, "milliseconds").unwrap(), 750);
    }

    #[test]
    fn rejects_bad_unit() {
        assert!(offset_to_ms(1.0, "minutes").is_err());
    }

    /// Drift guard: the descriptor-derived chat schema must match this authored
    /// schema, so any future change to the LLM-facing API is intentional.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "srt": { "type": "string", "description": "The SubRip (.srt) subtitle text to retime, including the cue numbers, the 'HH:MM:SS,mmm --> HH:MM:SS,mmm' timing lines, and the dialogue." },
                    "offset": { "type": "number", "description": "How far to shift every timestamp. Positive = later (delay subtitles), negative = earlier (advance them). Interpreted in `unit`." },
                    "unit": { "type": "string", "enum": ["seconds", "milliseconds"], "default": "seconds", "description": "The unit of `offset`: 'seconds' (default, fractions allowed e.g. 2.5) or 'milliseconds'." }
                },
                "required": ["srt", "offset"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
