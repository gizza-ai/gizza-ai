//! gizza-ai/midi-to-json — chat skill block on the shared tool abstraction.
//! Parse a Standard MIDI File (SMF, `.mid`/`.midi`) supplied as base64/hex bytes
//! into structured JSON: a musical `notes` view (paired note-on/off with pitch
//! names + ticks & seconds), a loss-less `events` stream, or a compact `summary`.
//! The chat schema is single-sourced from descriptor() (which also drives the
//! CLI); handle() delegates to run_skill. Pure → runs on all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    input: String,
    #[serde(default = "default_encoding")]
    encoding: String,
    #[serde(default = "default_format")]
    format: String,
}

fn default_encoding() -> String {
    "auto".to_string()
}
fn default_format() -> String {
    "notes".to_string()
}

/// Single source for the chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("input")
                .required()
                .describe("The Standard MIDI File (.mid/.midi) bytes as a base64 or hex string. The data must start with a standard `MThd` header chunk (Format 0/1/2). Whitespace, and `:`/`-` separators in hex, are ignored."),
        )
        .param(
            Param::enumv("encoding", ["auto", "base64", "hex"])
                .default("auto")
                .describe("How `input` is encoded: 'auto' (default) reads it as hex when it is all hex digits with an even length, otherwise base64; 'base64'; or 'hex'."),
        )
        .param(
            Param::enumv("format", ["notes", "events", "summary"])
                .default("notes")
                .describe("Output shape: 'notes' (default) a musical view — a header (format, PPQ/timecode division, tempo/time/key-signature maps, duration) plus per-track paired notes each with a pitch name (e.g. C4), MIDI number, channel, start & duration in ticks AND seconds, and velocity; 'events' a loss-less per-track raw event stream (every note-on/off, control change, program change, pitch bend, and meta event with delta/absolute ticks & seconds); 'summary' a compact overview (format, PPQ, BPM, duration, note/CC counts, channels, time & key signature, track names)."),
        )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/midi-to-json",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Parse a Standard MIDI File (base64/hex) into structured JSON — notes, raw events, or a summary",
    skill(
        description = "Parse a Standard MIDI File (SMF, .mid/.midi) into structured JSON. `input` is the MIDI bytes as a base64 or hex string (it must start with a standard `MThd` header chunk; Format 0/1/2). `encoding` is auto (default) / base64 / hex — auto reads hex when the input is all hex digits with an even length, else base64. `format` picks the output shape: 'notes' (default) a musical view — a header (format, PPQ or SMPTE-timecode division, tempo/time-signature/key-signature maps, duration in ticks and seconds) plus per-track paired notes, each with a scientific pitch name (e.g. C4), MIDI number, channel, start and duration in BOTH ticks and seconds (computed from a tempo map across all tracks), and raw 0–127 velocity; 'events' a loss-less per-track raw event stream (every note-on/off, control change, program change, pitch bend, aftertouch, sysex, and meta event with delta and absolute ticks and seconds); 'summary' a compact overview (format, PPQ, initial BPM, tempo-change count, duration, note and control-change counts, channels, first time and key signature, track names). Read-only — nothing is fetched, persisted, or modified. Runs locally.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "midi-to-json", |a: Args| {
            gizza_ai_midi_to_json_core::convert(&a.input, &a.encoding, &a.format)
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
    /// schema, so any future change to the LLM-facing API is intentional.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "input": { "type": "string", "description": "The Standard MIDI File (.mid/.midi) bytes as a base64 or hex string. The data must start with a standard `MThd` header chunk (Format 0/1/2). Whitespace, and `:`/`-` separators in hex, are ignored." },
                    "encoding": { "type": "string", "enum": ["auto", "base64", "hex"], "default": "auto", "description": "How `input` is encoded: 'auto' (default) reads it as hex when it is all hex digits with an even length, otherwise base64; 'base64'; or 'hex'." },
                    "format": { "type": "string", "enum": ["notes", "events", "summary"], "default": "notes", "description": "Output shape: 'notes' (default) a musical view — a header (format, PPQ/timecode division, tempo/time/key-signature maps, duration) plus per-track paired notes each with a pitch name (e.g. C4), MIDI number, channel, start & duration in ticks AND seconds, and velocity; 'events' a loss-less per-track raw event stream (every note-on/off, control change, program change, pitch bend, and meta event with delta/absolute ticks & seconds); 'summary' a compact overview (format, PPQ, BPM, duration, note/CC counts, channels, time & key signature, track names)." }
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
