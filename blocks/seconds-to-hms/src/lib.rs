//! gizza-ai/seconds-to-hms — converts a number of seconds into HH:MM:SS clock format.
//!
//! Thin chat-skill wrapper around `gizza-ai-seconds-to-hms-core`. The chat schema
//! is derived from `descriptor()` (single source — shared shape across chat + CLI);
//! the handler delegates to `block_utils::run_skill`. No host calls — runs entirely
//! inside the WASM sandbox.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    /// Total seconds — may be fractional or negative.
    seconds: f64,
    #[serde(default)]
    format: String,
    /// Fractional-second digits to show; the core clamps to 0..=9.
    #[serde(default)]
    decimals: u32,
}

/// Single-source param descriptor → chat schema (and CLI). See
/// docs/superpowers/specs/2026-06-19-gizza-shared-tool-abstraction-design.md.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::number("seconds")
                .required()
                .describe("The number of seconds to convert. May be fractional (e.g. 90.5) or negative."),
        )
        .param(
            Param::enumv("format", ["hms", "dhms", "auto", "iso", "words"])
                .default("hms")
                .describe("Output layout. 'hms' (default) is HH:MM:SS with the hours field accumulating past 24 (e.g. 90061 -> 25:01:01); 'dhms' splits days out as D:HH:MM:SS (e.g. 1:01:01:01); 'auto' is the shortest clock form, dropping leading zero days/hours (MM:SS, HH:MM:SS, or D:HH:MM:SS); 'iso' is an ISO-8601 duration (e.g. P1DT1H1M1S, zero -> PT0S); 'words' is human-readable (e.g. '1 hour, 1 minute, 1 second')."),
        )
        .param(
            // Bounds reference the core clamp (MAX_DECIMALS) so the LLM-facing
            // schema can't drift from what `to_hms` actually enforces.
            Param::integer("decimals")
                .default(0)
                .min(0.0)
                .max(gizza_ai_seconds_to_hms_core::MAX_DECIMALS as f64)
                .describe("Fractional-second digits to append, 0-9. E.g. 90.5 with decimals=1 shows '...30.5'. Default 0 (whole seconds)."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct SecondsToHms;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/seconds-to-hms",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Seconds to HH:MM:SS skill",
    skill(
        description = "Convert a number of seconds into a duration string. format='hms' (default) gives HH:MM:SS with the hours field accumulating past 24 (e.g. 90061 -> 25:01:01); format='dhms' splits out days as D:HH:MM:SS (e.g. 1:01:01:01); format='auto' returns the shortest clock form, dropping leading zero days/hours (MM:SS, HH:MM:SS, or D:HH:MM:SS); format='iso' returns an ISO-8601 duration (e.g. P1DT1H1M1S, zero -> PT0S); format='words' returns human-readable text (e.g. '1 hour, 1 minute, 1 second'). seconds may be fractional or negative (a negative input is '-'-prefixed); set decimals=1..9 to append fractional seconds (e.g. 90.5 -> 00:01:30.5).",
        parameters = schema_json()
    ),
)]
impl SecondsToHms {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        // run_skill wraps the returned value in { "result": … }.
        match run_skill(&body, "seconds-to-hms", |a: Args| {
            gizza_ai_seconds_to_hms_core::to_hms(a.seconds, &a.format, a.decimals)
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
    /// schema, so any future change to the LLM-facing API is intentional and reviewed.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "seconds": { "type": "number", "description": "The number of seconds to convert. May be fractional (e.g. 90.5) or negative." },
                    "format": { "type": "string", "enum": ["hms", "dhms", "auto", "iso", "words"], "default": "hms", "description": "Output layout. 'hms' (default) is HH:MM:SS with the hours field accumulating past 24 (e.g. 90061 -> 25:01:01); 'dhms' splits days out as D:HH:MM:SS (e.g. 1:01:01:01); 'auto' is the shortest clock form, dropping leading zero days/hours (MM:SS, HH:MM:SS, or D:HH:MM:SS); 'iso' is an ISO-8601 duration (e.g. P1DT1H1M1S, zero -> PT0S); 'words' is human-readable (e.g. '1 hour, 1 minute, 1 second')." },
                    "decimals": { "type": "integer", "minimum": 0, "maximum": 9, "default": 0, "description": "Fractional-second digits to append, 0-9. E.g. 90.5 with decimals=1 shows '...30.5'. Default 0 (whole seconds)." }
                },
                "required": ["seconds"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
