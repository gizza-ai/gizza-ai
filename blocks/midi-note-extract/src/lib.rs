//! gizza-ai/midi-note-extract — chat skill block on the shared tool abstraction.
//! Flatten a Standard MIDI File (SMF, `.mid`/`.midi`) supplied as base64/hex
//! bytes into one delimited note table: a row per note with track, channel,
//! start, duration, pitch, note name and velocity. The chat schema is
//! single-sourced from descriptor() (which also drives the CLI); handle()
//! delegates to run_skill. Pure → runs on all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_midi_note_extract_core::Options;
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    input: String,
    #[serde(default = "default_encoding")]
    encoding: String,
    #[serde(default = "default_columns")]
    columns: String,
    #[serde(default = "default_time_unit")]
    time_unit: String,
    #[serde(default = "default_velocity_scale")]
    velocity_scale: String,
    #[serde(default = "default_delimiter")]
    delimiter: String,
    #[serde(default = "default_header")]
    header: bool,
    #[serde(default = "default_sort")]
    sort: String,
    #[serde(default = "default_all")]
    track: String,
    #[serde(default = "default_all")]
    channel: String,
    #[serde(default = "default_decimals")]
    decimals: f64,
}

fn default_encoding() -> String {
    "auto".to_string()
}
fn default_columns() -> String {
    "standard".to_string()
}
fn default_time_unit() -> String {
    "seconds".to_string()
}
fn default_velocity_scale() -> String {
    "raw".to_string()
}
fn default_delimiter() -> String {
    "comma".to_string()
}
fn default_header() -> bool {
    true
}
fn default_sort() -> String {
    "time".to_string()
}
fn default_all() -> String {
    "all".to_string()
}
fn default_decimals() -> f64 {
    3.0
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
            Param::enumv("columns", ["minimal", "standard", "full"])
                .default("standard")
                .describe("Which columns each row carries: 'minimal' = start, duration, pitch, velocity; 'standard' (default) = track, channel, start, duration, pitch, note_name, velocity; 'full' also adds track_name, end and tempo_bpm (the tempo in force when the note starts)."),
        )
        .param(
            Param::enumv("time_unit", ["seconds", "ticks", "beats"])
                .default("seconds")
                .describe("Unit for the start/end/duration columns: 'seconds' (default) from the file's tempo map, 'ticks' (raw whole MIDI ticks), or 'beats' (quarter notes; needs a metrical PPQ file, not SMPTE timecode)."),
        )
        .param(
            Param::enumv("velocity_scale", ["raw", "normalized"])
                .default("raw")
                .describe("Velocity column scale: 'raw' (default) the MIDI 0-127 integer, or 'normalized' the same value divided by 127 as a 0.0-1.0 decimal."),
        )
        .param(
            Param::enumv("delimiter", ["comma", "semicolon", "tab"])
                .default("comma")
                .describe("Column separator: 'comma' (default, CSV), 'semicolon', or 'tab' (TSV). Track names containing the separator, a quote or a newline are quoted RFC 4180 style."),
        )
        .param(
            Param::boolean("header")
                .default(true)
                .describe("Write the column-name header row first. Default true; set false for a bare data table."),
        )
        .param(
            Param::enumv("sort", ["time", "track", "pitch"])
                .default("time")
                .describe("Row order: 'time' (default) chronological across the whole file, 'track' grouped by track then chronological, or 'pitch' lowest note first then chronological."),
        )
        .param(
            Param::string("track")
                .default("all")
                .describe("Which tracks to include: 'all' (default) or a comma-separated list of 0-based track numbers, e.g. '1' or '0,2'."),
        )
        .param(
            Param::string("channel")
                .default("all")
                .describe("Which MIDI channels to include: 'all' (default) or a comma-separated list of 0-based channel numbers, e.g. '0' or '0,9' (channel 9 is General MIDI drums)."),
        )
        .param(
            Param::integer("decimals")
                .default(3)
                .min(0.0)
                .max(6.0)
                .describe("Decimal places for the fractional columns (seconds/beats times, normalized velocity, tempo). Default 3. Tick values are always whole numbers."),
        )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/midi-note-extract",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Extract every note from a MIDI file as a CSV/TSV table of pitch, start, duration and velocity",
    skill(
        description = "Extract every note from a Standard MIDI File (SMF, .mid/.midi) as one delimited table — a row per note with pitch, start, duration and velocity, ready for a spreadsheet, pandas or a plotting script. `input` is the MIDI bytes as a base64 or hex string (it must start with a standard `MThd` header chunk; Format 0/1/2), and `encoding` is auto (default) / base64 / hex. Note-on and note-off messages are paired per channel and pitch, every track is flattened into one table, and a note left unclosed by the file is held to the end of its track. `columns` picks the row shape: 'minimal' (start, duration, pitch, velocity), 'standard' (default: track, channel, start, duration, pitch, note_name, velocity) or 'full' (also track_name, end and tempo_bpm). `time_unit` is seconds (default, computed from the file's tempo map), ticks, or beats (quarter notes; metrical PPQ files only). `velocity_scale` is raw 0-127 (default) or normalized 0.0-1.0. `delimiter` is comma (default), semicolon or tab; `header` toggles the column-name row; `sort` is time (default), track or pitch; `track` and `channel` filter to 'all' (default) or a comma-separated list of 0-based numbers; `decimals` (default 3) sets the fractional precision. Returns at most 50000 rows. Read-only — nothing is fetched, persisted, or modified. Runs locally.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "midi-note-extract", |a: Args| {
            let opts = Options::parse(
                &a.columns,
                &a.time_unit,
                &a.velocity_scale,
                &a.delimiter,
                a.header,
                &a.track,
                &a.channel,
                a.decimals as i64,
                &a.sort,
            )
            .map_err(SkillError::InvalidArgs)?;
            gizza_ai_midi_note_extract_core::extract(&a.input, &a.encoding, &opts)
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
                    "columns": { "type": "string", "enum": ["minimal", "standard", "full"], "default": "standard", "description": "Which columns each row carries: 'minimal' = start, duration, pitch, velocity; 'standard' (default) = track, channel, start, duration, pitch, note_name, velocity; 'full' also adds track_name, end and tempo_bpm (the tempo in force when the note starts)." },
                    "time_unit": { "type": "string", "enum": ["seconds", "ticks", "beats"], "default": "seconds", "description": "Unit for the start/end/duration columns: 'seconds' (default) from the file's tempo map, 'ticks' (raw whole MIDI ticks), or 'beats' (quarter notes; needs a metrical PPQ file, not SMPTE timecode)." },
                    "velocity_scale": { "type": "string", "enum": ["raw", "normalized"], "default": "raw", "description": "Velocity column scale: 'raw' (default) the MIDI 0-127 integer, or 'normalized' the same value divided by 127 as a 0.0-1.0 decimal." },
                    "delimiter": { "type": "string", "enum": ["comma", "semicolon", "tab"], "default": "comma", "description": "Column separator: 'comma' (default, CSV), 'semicolon', or 'tab' (TSV). Track names containing the separator, a quote or a newline are quoted RFC 4180 style." },
                    "header": { "type": "boolean", "default": true, "description": "Write the column-name header row first. Default true; set false for a bare data table." },
                    "sort": { "type": "string", "enum": ["time", "track", "pitch"], "default": "time", "description": "Row order: 'time' (default) chronological across the whole file, 'track' grouped by track then chronological, or 'pitch' lowest note first then chronological." },
                    "track": { "type": "string", "default": "all", "description": "Which tracks to include: 'all' (default) or a comma-separated list of 0-based track numbers, e.g. '1' or '0,2'." },
                    "channel": { "type": "string", "default": "all", "description": "Which MIDI channels to include: 'all' (default) or a comma-separated list of 0-based channel numbers, e.g. '0' or '0,9' (channel 9 is General MIDI drums)." },
                    "decimals": { "type": "integer", "default": 3, "minimum": 0, "maximum": 6, "description": "Decimal places for the fractional columns (seconds/beats times, normalized velocity, tempo). Default 3. Tick values are always whole numbers." }
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
