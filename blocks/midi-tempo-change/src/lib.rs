//! gizza-ai/midi-tempo-change — change the tempo/BPM of a Standard MIDI File
//! while every note stays exactly where it is musically. Pure compute with
//! BINARY output, so `handle()` builds a base64 download envelope like
//! guitar-tab-to-midi rather than using `run_skill`. The chat schema is
//! single-sourced from `descriptor()` (which also drives the CLI); the page
//! calls the same core through `web/`.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
use gizza_ai_block_utils::{Envelope, ForUi, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_midi_tempo_change_core::{change_tempo, Options};
use serde::Deserialize;
use wafer_sdk::*;

/// Standard MIDI File MIME type.
const MIDI_MIME: &str = "audio/midi";
/// Download name for the retimed file.
const MIDI_FILENAME: &str = "tempo-changed.mid";

#[derive(Deserialize, Debug)]
#[serde(default)]
struct Args {
    input: String,
    encoding: String,
    mode: String,
    bpm: f64,
    factor: f64,
    tempo_map: String,
    keep_duration: bool,
}

impl Default for Args {
    fn default() -> Self {
        Args {
            input: String::new(),
            encoding: "auto".to_string(),
            mode: "set-bpm".to_string(),
            bpm: 120.0,
            factor: 1.0,
            tempo_map: "scale".to_string(),
            keep_duration: false,
        }
    }
}

/// Single source for the chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("input")
                .required()
                .describe("The Standard MIDI File (.mid/.midi) bytes as a base64 or hex string. The data must start with a standard `MThd` header chunk (Format 0/1/2), use metrical ticks-per-quarter-note timing, and be at most 4 MiB. Whitespace, and `:`/`-` separators in hex, are ignored."),
        )
        .param(
            Param::enumv("encoding", ["auto", "base64", "hex"])
                .default("auto")
                .describe("How `input` is encoded: 'auto' (default) reads it as hex when it is all hex digits with an even length, otherwise base64; 'base64'; or 'hex'."),
        )
        .param(
            Param::enumv("mode", ["set-bpm", "scale"])
                .default("set-bpm")
                .describe("How the new tempo is chosen: 'set-bpm' (default) targets the absolute `bpm` value, so the file's FIRST tempo becomes it; 'scale' multiplies the existing tempo by `factor` (use it when you want '20% faster' rather than a specific number)."),
        )
        .param(
            Param::number("bpm")
                .default(120.0)
                .min(20.0)
                .max(400.0)
                .describe("Target tempo in quarter-note beats per minute when `mode` is 'set-bpm' (default 120, range 20-400). Decimals are allowed, e.g. 128.5. Ignored when `mode` is 'scale'."),
        )
        .param(
            Param::number("factor")
                .default(1.0)
                .min(0.1)
                .max(10.0)
                .describe("Speed multiplier when `mode` is 'scale' (default 1.0 = no change, range 0.1-10). 2.0 plays twice as fast, 0.5 half speed, 1.25 is '125% speed'. Ignored when `mode` is 'set-bpm'."),
        )
        .param(
            Param::enumv("tempo_map", ["scale", "flatten"])
                .default("scale")
                .describe("What to do with a file that already changes tempo as it plays: 'scale' (default) multiplies EVERY tempo event by the same ratio, so ritardandos and accelerandos keep their shape; 'flatten' deletes the whole tempo map and writes one constant tempo at the start."),
        )
        .param(
            Param::boolean("keep_duration")
                .default(false)
                .describe("Re-notate instead of retiming: when true (default false) every event tick is rescaled by the same ratio, so the file still plays for exactly as long as before at the new BPM — a quarter note at 120 becomes a half note at 240. Use it to match a project's BPM without changing how the part sounds."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/midi-tempo-change",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Change the tempo or BPM of a MIDI file while every note keeps its musical position",
    skill(
        description = "Change the tempo/BPM of a Standard MIDI File (.mid) and return the retimed file as a download. `input` is the file's bytes as base64 or hex. Notes are never touched: every note-on, note-off, velocity, controller, program change, track name and time signature is carried through unchanged, and only the tempo meta events (microseconds per quarter note) are rewritten, so the relationships between notes survive exactly. `mode` picks how the new tempo is chosen — 'set-bpm' targets an absolute `bpm` (the file's first tempo becomes it) and 'scale' multiplies the existing tempo by `factor` (2.0 = twice as fast, 0.5 = half speed). `tempo_map` decides what happens to a file that already changes tempo while it plays: 'scale' multiplies every tempo event by the same ratio so ritardandos keep their shape, 'flatten' replaces the map with one constant tempo. `keep_duration` re-notates instead of retiming — every tick is rescaled too, so the file plays for the same length of time at the new BPM (a quarter note at 120 becomes a half note at 240), which is how you match a part to a project's BPM without changing how it sounds. A file with no tempo event is treated as the MIDI default 120 BPM and gets one written. SMPTE timecode files are rejected, since their speed comes from the frame rate, not tempo events. Pure compute — nothing is fetched, uploaded or stored.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    let args: Args = serde_json::from_slice(&body)
        .map_err(|e| SkillError::InvalidArgs(format!("invalid midi-tempo-change args: {e}")))?;
    let opts = Options::parse(
        &args.mode,
        args.bpm,
        args.factor,
        &args.tempo_map,
        args.keep_duration,
    )
    .map_err(SkillError::InvalidArgs)?;
    let out =
        change_tempo(&args.input, &args.encoding, &opts).map_err(SkillError::InvalidArgs)?;

    let env = Envelope {
        for_llm: out.summary(),
        for_ui: ForUi {
            data_url: format!("data:{MIDI_MIME};base64,{}", B64.encode(&out.midi)),
            mime: MIDI_MIME.to_string(),
            filename: MIDI_FILENAME.to_string(),
        },
    };
    serde_json::to_vec(&env).map_err(|e| SkillError::Serialize(format!("serialize envelope: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Drift guard: the descriptor-derived chat schema must match this authored
    /// copy, so an accidental descriptor edit can't silently change the
    /// LLM-facing schema (or the page controls the manifest renders from it).
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "input": { "type": "string", "description": "The Standard MIDI File (.mid/.midi) bytes as a base64 or hex string. The data must start with a standard `MThd` header chunk (Format 0/1/2), use metrical ticks-per-quarter-note timing, and be at most 4 MiB. Whitespace, and `:`/`-` separators in hex, are ignored." },
                    "encoding": { "type": "string", "enum": ["auto", "base64", "hex"], "default": "auto", "description": "How `input` is encoded: 'auto' (default) reads it as hex when it is all hex digits with an even length, otherwise base64; 'base64'; or 'hex'." },
                    "mode": { "type": "string", "enum": ["set-bpm", "scale"], "default": "set-bpm", "description": "How the new tempo is chosen: 'set-bpm' (default) targets the absolute `bpm` value, so the file's FIRST tempo becomes it; 'scale' multiplies the existing tempo by `factor` (use it when you want '20% faster' rather than a specific number)." },
                    "bpm": { "type": "number", "default": 120.0, "minimum": 20, "maximum": 400, "description": "Target tempo in quarter-note beats per minute when `mode` is 'set-bpm' (default 120, range 20-400). Decimals are allowed, e.g. 128.5. Ignored when `mode` is 'scale'." },
                    "factor": { "type": "number", "default": 1.0, "minimum": 0.1, "maximum": 10, "description": "Speed multiplier when `mode` is 'scale' (default 1.0 = no change, range 0.1-10). 2.0 plays twice as fast, 0.5 half speed, 1.25 is '125% speed'. Ignored when `mode` is 'set-bpm'." },
                    "tempo_map": { "type": "string", "enum": ["scale", "flatten"], "default": "scale", "description": "What to do with a file that already changes tempo as it plays: 'scale' (default) multiplies EVERY tempo event by the same ratio, so ritardandos and accelerandos keep their shape; 'flatten' deletes the whole tempo map and writes one constant tempo at the start." },
                    "keep_duration": { "type": "boolean", "default": false, "description": "Re-notate instead of retiming: when true (default false) every event tick is rescaled by the same ratio, so the file still plays for exactly as long as before at the new BPM — a quarter note at 120 becomes a half note at 240. Use it to match a project's BPM without changing how the part sounds." }
                },
                "required": ["input"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }

    /// The serde defaults the chat/CLI surface applies must be the same ones the
    /// core (and therefore the page) treats as default.
    #[test]
    fn args_defaults_match_the_core_defaults() {
        let args: Args = serde_json::from_str(r#"{"input":"4d546864"}"#).unwrap();
        let o = Options::default();
        let parsed = Options::parse(
            &args.mode,
            args.bpm,
            args.factor,
            &args.tempo_map,
            args.keep_duration,
        )
        .unwrap();
        assert_eq!(parsed.mode, o.mode);
        assert_eq!(parsed.tempo_map, o.tempo_map);
        assert_eq!(parsed.keep_duration, o.keep_duration);
        assert_eq!(parsed.bpm, o.bpm);
        assert_eq!(parsed.factor, o.factor);
        assert_eq!(args.encoding, "auto");
    }
}
