//! gizza-ai/tempo-map-extractor — chat skill block on the shared tool abstraction.
//! Turns a list of beat times into a tempo map: the BPM-versus-time curve of a
//! performance, its statistics, and DAW-ready exports. The chat schema is
//! single-sourced from descriptor() (which also drives the CLI); handle()
//! delegates to run_skill. Pure → runs on all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_tempo_map_extractor_core::{extract, Spec};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    beats: String,
    #[serde(default = "default_time_unit")]
    time_unit: String,
    #[serde(default = "default_fps")]
    fps: f64,
    #[serde(default = "default_beat_unit")]
    beat_unit: String,
    #[serde(default = "default_smoothing")]
    smoothing: usize,
    #[serde(default = "default_smooth_method")]
    smooth_method: String,
    #[serde(default)]
    grid_seconds: f64,
    #[serde(default)]
    min_interval_ms: f64,
    #[serde(default)]
    offset_seconds: f64,
    #[serde(default = "default_decimals")]
    decimals: usize,
    #[serde(default = "default_output")]
    output: String,
    #[serde(default = "default_ppq")]
    ppq: usize,
}

fn default_time_unit() -> String {
    "auto".into()
}
fn default_fps() -> f64 {
    30.0
}
fn default_beat_unit() -> String {
    "quarter".into()
}
fn default_smoothing() -> usize {
    1
}
fn default_smooth_method() -> String {
    "mean".into()
}
fn default_decimals() -> usize {
    2
}
fn default_output() -> String {
    "csv".into()
}
fn default_ppq() -> usize {
    960
}

/// Single source for the chat schema (and the CLI, the page form and the
/// generated CLI examples).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("beats").required().describe("The beat times, one per line — a beat-tracker export, a DAW marker list, an Audacity label track, a CSV column or tapped timestamps. Each line's first field is used, so extra columns are ignored; blank lines, a header row, and # or // comments are skipped. A single comma-separated line (0, 0.5, 1) is also accepted. Times may be decimal seconds (1.75), a unit-suffixed value (1750ms), m:ss.mmm (0:01.750), h:mm:ss.mmm, or hh:mm:ss:ff frame timecode. At least 2 and at most 20000 beats; they must increase."))
        .param(Param::enumv("time_unit", ["auto", "seconds", "milliseconds"]).default("auto").describe("How to read a plain number with no unit suffix: 'auto' (default) and 'seconds' treat it as seconds; 'milliseconds' treats it as milliseconds. Colon timecodes and explicit ms/s suffixes are always honoured whatever this is set to. Default auto."))
        .param(Param::number("fps").default(30.0).min(1.0).max(240.0).describe("Frame rate used only when a beat time is written as hh:mm:ss:ff frame timecode, so the last field can be converted to seconds. Common values are 24, 25, 29.97, 30 and 60. Ignored by every other time format. Default 30."))
        .param(Param::enumv("beat_unit", ["whole", "dotted-half", "half", "dotted-quarter", "quarter", "dotted-eighth", "eighth", "triplet-eighth", "sixteenth"]).default("quarter").describe("The note value each supplied beat represents, used to convert the reading to standard quarter-note BPM. 'quarter' (default) is a normal beat. Use this to fix a half/double-time reading: 'half' doubles the BPM (you marked every other beat), 'eighth' halves it (you marked twice per beat). Dotted and triplet-eighth pulses are supported for compound and swung material."))
        .param(Param::integer("smoothing").default(1).min(1.0).max(64.0).describe("Width, in beats, of a centred moving window applied to the tempo curve, 1-64. 1 (default) keeps every raw beat-to-beat reading; larger values flatten tapping jitter so the underlying tempo shape is readable. Try 4-8 for hand-tapped input."))
        .param(Param::enumv("smooth_method", ["mean", "median"]).default("mean").describe("How the smoothing window is combined: 'mean' (default) averages the window; 'median' takes the middle value, which ignores a single badly-placed beat instead of letting it drag the curve. Ignored when smoothing is 1."))
        .param(Param::number("grid_seconds").default(0.0).min(0.0).max(3600.0).describe("Resample the curve onto an even time grid instead of one row per beat: the number of seconds between rows, for example 1 for a reading every second. Each grid row holds the tempo of the beat interval it falls inside. 0 (default) emits one row per beat. Cannot be used with output=midi."))
        .param(Param::number("min_interval_ms").default(0.0).min(0.0).max(10000.0).describe("Drop any beat that lands closer than this many milliseconds to the previous kept beat — the double-tap guard for hand-tapped input, and the way to remove duplicated markers. 0 (default) keeps every beat. 80-200 suits tapping."))
        .param(Param::number("offset_seconds").default(0.0).describe("Seconds added to every beat time before anything else, so the map lines up with a project timeline. Negative values shift earlier, for example -2.5 when your export started 2.5 s after the session start. Default 0."))
        .param(Param::integer("decimals").default(2).min(0.0).max(4.0).describe("Digits after the decimal point for BPM values, 0-4. Default 2. Times are always given to millisecond precision."))
        .param(Param::enumv("output", ["csv", "tsv", "json", "table", "audacity", "midi", "summary"]).default("csv").describe("Result format. 'csv' (default) and 'tsv' emit time_seconds, bpm, beat and interval_ms columns ready to plot; 'json' returns the whole map plus a summary object; 'table' is an aligned human-readable ledger with a per-beat deviation column and a statistics footer; 'audacity' emits a tab-separated label track (start, end, BPM) you can import back into a label editor; 'midi' emits Standard-MIDI-File tempo-map rows (tick, microseconds_per_quarter, bpm) with one event per tempo change; 'summary' reports only the statistics."))
        .param(Param::integer("ppq").default(960).min(24.0).max(15360.0).describe("Ticks per quarter note used to place the tick column of the output=midi tempo map, 24-15360. Match your DAW or MIDI file's division — 480 and 960 are the usual values. Ignored by every other output format. Default 960."))
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/tempo-map-extractor",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Turn a list of beat times into a BPM-versus-time tempo map with statistics and DAW exports",
    skill(
        description = "Build a tempo map from a list of beat times: the BPM-versus-time curve of a performance rather than one global tempo. `beats` is the beat timestamps, one per line — a beat-tracker or DAW marker export, an Audacity label track, a CSV column, or tapped times; each line's first field is used, headers/blank lines/# comments are skipped, and a single comma-separated line works too. Times may be decimal seconds, unit-suffixed values (1750ms), m:ss.mmm, h:mm:ss.mmm or hh:mm:ss:ff frame timecode (`fps` sets the frame rate; `time_unit` decides whether a bare number is seconds or milliseconds). Every consecutive pair of beats gives one instantaneous tempo, so N beats yield N-1 readings. `beat_unit` converts the marked pulse to standard quarter-note BPM and fixes half/double-time readings. `smoothing` plus `smooth_method` apply a centred moving mean or median to flatten tapping jitter, `min_interval_ms` drops double taps, and `offset_seconds` aligns the map to a project timeline. `grid_seconds` resamples the curve onto an even time grid. `output` selects csv/tsv columns ready to plot, json with a summary object, an aligned table ledger, an Audacity label track, a Standard-MIDI-File tempo map (tick, microseconds per quarter, bpm at `ppq` ticks per quarter), or a statistics-only summary reporting mean/median/min/max BPM, drift, standard deviation, interval jitter, the overall average tempo, the trend in BPM per minute, a stability rating and the conventional tempo marking. Pure compute — nothing is fetched, uploaded or stored.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "tempo-map-extractor", |a: Args| {
            extract(&Spec {
                beats: &a.beats,
                time_unit: &a.time_unit,
                fps: a.fps,
                beat_unit: &a.beat_unit,
                smoothing: a.smoothing,
                smooth_method: &a.smooth_method,
                grid_seconds: a.grid_seconds,
                min_interval_ms: a.min_interval_ms,
                offset_seconds: a.offset_seconds,
                decimals: a.decimals,
                output: &a.output,
                ppq: a.ppq,
            })
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
                    "beats": { "type": "string", "description": "The beat times, one per line — a beat-tracker export, a DAW marker list, an Audacity label track, a CSV column or tapped timestamps. Each line's first field is used, so extra columns are ignored; blank lines, a header row, and # or // comments are skipped. A single comma-separated line (0, 0.5, 1) is also accepted. Times may be decimal seconds (1.75), a unit-suffixed value (1750ms), m:ss.mmm (0:01.750), h:mm:ss.mmm, or hh:mm:ss:ff frame timecode. At least 2 and at most 20000 beats; they must increase." },
                    "time_unit": { "type": "string", "enum": ["auto", "seconds", "milliseconds"], "default": "auto", "description": "How to read a plain number with no unit suffix: 'auto' (default) and 'seconds' treat it as seconds; 'milliseconds' treats it as milliseconds. Colon timecodes and explicit ms/s suffixes are always honoured whatever this is set to. Default auto." },
                    "fps": { "type": "number", "minimum": 1, "maximum": 240, "default": 30.0, "description": "Frame rate used only when a beat time is written as hh:mm:ss:ff frame timecode, so the last field can be converted to seconds. Common values are 24, 25, 29.97, 30 and 60. Ignored by every other time format. Default 30." },
                    "beat_unit": { "type": "string", "enum": ["whole", "dotted-half", "half", "dotted-quarter", "quarter", "dotted-eighth", "eighth", "triplet-eighth", "sixteenth"], "default": "quarter", "description": "The note value each supplied beat represents, used to convert the reading to standard quarter-note BPM. 'quarter' (default) is a normal beat. Use this to fix a half/double-time reading: 'half' doubles the BPM (you marked every other beat), 'eighth' halves it (you marked twice per beat). Dotted and triplet-eighth pulses are supported for compound and swung material." },
                    "smoothing": { "type": "integer", "minimum": 1, "maximum": 64, "default": 1, "description": "Width, in beats, of a centred moving window applied to the tempo curve, 1-64. 1 (default) keeps every raw beat-to-beat reading; larger values flatten tapping jitter so the underlying tempo shape is readable. Try 4-8 for hand-tapped input." },
                    "smooth_method": { "type": "string", "enum": ["mean", "median"], "default": "mean", "description": "How the smoothing window is combined: 'mean' (default) averages the window; 'median' takes the middle value, which ignores a single badly-placed beat instead of letting it drag the curve. Ignored when smoothing is 1." },
                    "grid_seconds": { "type": "number", "minimum": 0, "maximum": 3600, "default": 0.0, "description": "Resample the curve onto an even time grid instead of one row per beat: the number of seconds between rows, for example 1 for a reading every second. Each grid row holds the tempo of the beat interval it falls inside. 0 (default) emits one row per beat. Cannot be used with output=midi." },
                    "min_interval_ms": { "type": "number", "minimum": 0, "maximum": 10000, "default": 0.0, "description": "Drop any beat that lands closer than this many milliseconds to the previous kept beat — the double-tap guard for hand-tapped input, and the way to remove duplicated markers. 0 (default) keeps every beat. 80-200 suits tapping." },
                    "offset_seconds": { "type": "number", "default": 0.0, "description": "Seconds added to every beat time before anything else, so the map lines up with a project timeline. Negative values shift earlier, for example -2.5 when your export started 2.5 s after the session start. Default 0." },
                    "decimals": { "type": "integer", "minimum": 0, "maximum": 4, "default": 2, "description": "Digits after the decimal point for BPM values, 0-4. Default 2. Times are always given to millisecond precision." },
                    "output": { "type": "string", "enum": ["csv", "tsv", "json", "table", "audacity", "midi", "summary"], "default": "csv", "description": "Result format. 'csv' (default) and 'tsv' emit time_seconds, bpm, beat and interval_ms columns ready to plot; 'json' returns the whole map plus a summary object; 'table' is an aligned human-readable ledger with a per-beat deviation column and a statistics footer; 'audacity' emits a tab-separated label track (start, end, BPM) you can import back into a label editor; 'midi' emits Standard-MIDI-File tempo-map rows (tick, microseconds_per_quarter, bpm) with one event per tempo change; 'summary' reports only the statistics." },
                    "ppq": { "type": "integer", "minimum": 24, "maximum": 15360, "default": 960, "description": "Ticks per quarter note used to place the tick column of the output=midi tempo map, 24-15360. Match your DAW or MIDI file's division — 480 and 960 are the usual values. Ignored by every other output format. Default 960." }
                },
                "required": ["beats"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }

    #[test]
    fn schema_has_no_todos() {
        assert!(!schema_json().contains("TODO"));
    }
}
