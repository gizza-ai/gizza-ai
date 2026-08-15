//! gizza-ai/midi-track-splitter — chat skill block on the shared tool abstraction.
//! Split one multi-track Standard MIDI File into separate single-part `.mid`
//! files, one per track or per MIDI channel, each carrying a copy of the
//! conductor data (tempo, time and key signature) so it plays correctly alone.
//! The chat schema is single-sourced from descriptor() (which also drives the
//! CLI); handle() delegates to run_skill and the page calls the same core
//! through `web/`. Pure → runs on all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_midi_track_splitter_core::{split_to_json, Options};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize, Debug)]
#[serde(default)]
struct Args {
    input: String,
    encoding: String,
    split_by: String,
    include_conductor: bool,
    output_format: String,
    skip_empty: bool,
    select: String,
    filename_prefix: String,
    output: String,
}

impl Default for Args {
    fn default() -> Self {
        Args {
            input: String::new(),
            encoding: "auto".to_string(),
            split_by: "track".to_string(),
            include_conductor: true,
            output_format: "format-0".to_string(),
            skip_empty: true,
            select: String::new(),
            filename_prefix: "part".to_string(),
            output: "files".to_string(),
        }
    }
}

/// Single source for the chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("input")
                .required()
                .describe("The Standard MIDI File (.mid/.midi) bytes as a base64 or hex string. The data must start with a standard `MThd` header chunk (Format 0/1/2) and be at most 4 MiB. Whitespace, and `:`/`-` separators in hex, are ignored."),
        )
        .param(
            Param::enumv("encoding", ["auto", "base64", "hex"])
                .default("auto")
                .describe("How `input` is encoded: 'auto' (default) reads it as hex when it is all hex digits with an even length, otherwise base64; 'base64'; or 'hex'."),
        )
        .param(
            Param::enumv("split_by", ["track", "channel"])
                .default("track")
                .describe("What each output file holds: 'track' (default) one file per track chunk, the usual cut for a Format 1 file exported by a DAW; 'channel' one file per MIDI channel, gathering that channel's events from every track. A Format 0 file has only one track, so it is always split by channel."),
        )
        .param(
            Param::boolean("include_conductor")
                .default(true)
                .describe("Copy the conductor data — tempo, time signature, key signature and SMPTE offset — into every output file (default true). Leave it on: those events normally live in track 0 only, so a part exported without them plays back at the MIDI default 120 BPM in 4/4. Events a part already carries are never duplicated."),
        )
        .param(
            Param::enumv("output_format", ["format-0", "format-1"])
                .default("format-0")
                .describe("How each output file is written: 'format-0' (default) merges the conductor data and the part into ONE track, a genuinely single-track file; 'format-1' keeps the conductor data as its own first track with the part second. Both preserve the source division (ticks per quarter note or SMPTE timecode) exactly."),
        )
        .param(
            Param::boolean("skip_empty")
                .default(true)
                .describe("Skip parts that contain no notes (default true) — that is what drops the conductor/tempo track itself, and any muted or placeholder track. Set false to export every track, empty ones included."),
        )
        .param(
            Param::string("select")
                .default("")
                .describe("Which parts to export, as 1-based numbers and ranges, e.g. '1,3-5'. Empty (default) exports every part. The numbers are TRACK numbers when `split_by` is 'track' (track 1 is the first chunk in the file) and MIDI CHANNEL numbers 1-16 when it is 'channel'."),
        )
        .param(
            Param::string("filename_prefix")
                .default("part")
                .describe("First word of each suggested download name (default 'part'), which is built as `<prefix>-<number>-<part name>.mid`, e.g. 'part-02-bass.mid'. Anything that is not a letter or digit becomes a hyphen."),
        )
        .param(
            Param::enumv("output", ["files", "list"])
                .default("files")
                .describe("How much to return: 'files' (default) the table of parts plus each part's complete .mid bytes as a `data:audio/midi;base64,…` URL; 'list' only the table — part name, source track/channel, instrument, note count and length — which is the quick way to see what is inside a file before exporting it."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/midi-track-splitter",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Split a multi-track MIDI file into separate single-track .mid files, one per track or channel",
    skill(
        description = "Split one multi-track Standard MIDI File (.mid) into separate single-part MIDI files and return each as a downloadable `data:audio/midi;base64,…` URL alongside a table of what it contains. `input` is the file's bytes as base64 or hex. `split_by` picks the cut: 'track' (default) gives one file per track chunk — how a DAW-exported Format 1 file is organised — and 'channel' gives one file per MIDI channel, gathered across every track (a Format 0 file has a single track, so it is always cut by channel). Every note-on, note-off, velocity, controller, pitch bend and program change is carried through unchanged at its original tick, and the source division is preserved, so nothing is re-gridded. `include_conductor` (default true) copies the tempo, time signature, key signature and SMPTE offset into each part — without them an exported part plays at the MIDI default 120 BPM in 4/4 — and never duplicates an event the part already had. `output_format` writes each part as 'format-0' (one merged track, default) or 'format-1' (conductor track + part track). `skip_empty` (default true) drops parts with no notes, which is what removes the tempo-only conductor track. `select` exports a subset by 1-based track number or channel number, e.g. '1,3-5'. Filenames are built from each part's track name, or its General MIDI instrument, as `<filename_prefix>-<number>-<name>.mid`. Set `output` to 'list' to see the parts table without the file bytes. Limits: 4 MiB decoded input and at most 64 output files. Pure compute — nothing is fetched, uploaded or stored.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "midi-track-splitter", |a: Args| {
            let opts = Options::parse(
                &a.split_by,
                a.include_conductor,
                &a.output_format,
                a.skip_empty,
                &a.select,
                &a.filename_prefix,
                &a.output,
            )
            .map_err(SkillError::InvalidArgs)?;
            split_to_json(&a.input, &a.encoding, &opts).map_err(SkillError::InvalidArgs)
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
    /// copy, so an accidental descriptor edit can't silently change the
    /// LLM-facing schema (or the page controls the manifest renders from it).
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "input": { "type": "string", "description": "The Standard MIDI File (.mid/.midi) bytes as a base64 or hex string. The data must start with a standard `MThd` header chunk (Format 0/1/2) and be at most 4 MiB. Whitespace, and `:`/`-` separators in hex, are ignored." },
                    "encoding": { "type": "string", "enum": ["auto", "base64", "hex"], "default": "auto", "description": "How `input` is encoded: 'auto' (default) reads it as hex when it is all hex digits with an even length, otherwise base64; 'base64'; or 'hex'." },
                    "split_by": { "type": "string", "enum": ["track", "channel"], "default": "track", "description": "What each output file holds: 'track' (default) one file per track chunk, the usual cut for a Format 1 file exported by a DAW; 'channel' one file per MIDI channel, gathering that channel's events from every track. A Format 0 file has only one track, so it is always split by channel." },
                    "include_conductor": { "type": "boolean", "default": true, "description": "Copy the conductor data — tempo, time signature, key signature and SMPTE offset — into every output file (default true). Leave it on: those events normally live in track 0 only, so a part exported without them plays back at the MIDI default 120 BPM in 4/4. Events a part already carries are never duplicated." },
                    "output_format": { "type": "string", "enum": ["format-0", "format-1"], "default": "format-0", "description": "How each output file is written: 'format-0' (default) merges the conductor data and the part into ONE track, a genuinely single-track file; 'format-1' keeps the conductor data as its own first track with the part second. Both preserve the source division (ticks per quarter note or SMPTE timecode) exactly." },
                    "skip_empty": { "type": "boolean", "default": true, "description": "Skip parts that contain no notes (default true) — that is what drops the conductor/tempo track itself, and any muted or placeholder track. Set false to export every track, empty ones included." },
                    "select": { "type": "string", "default": "", "description": "Which parts to export, as 1-based numbers and ranges, e.g. '1,3-5'. Empty (default) exports every part. The numbers are TRACK numbers when `split_by` is 'track' (track 1 is the first chunk in the file) and MIDI CHANNEL numbers 1-16 when it is 'channel'." },
                    "filename_prefix": { "type": "string", "default": "part", "description": "First word of each suggested download name (default 'part'), which is built as `<prefix>-<number>-<part name>.mid`, e.g. 'part-02-bass.mid'. Anything that is not a letter or digit becomes a hyphen." },
                    "output": { "type": "string", "enum": ["files", "list"], "default": "files", "description": "How much to return: 'files' (default) the table of parts plus each part's complete .mid bytes as a `data:audio/midi;base64,…` URL; 'list' only the table — part name, source track/channel, instrument, note count and length — which is the quick way to see what is inside a file before exporting it." }
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
        let d = Options::default();
        let parsed = Options::parse(
            &args.split_by,
            args.include_conductor,
            &args.output_format,
            args.skip_empty,
            &args.select,
            &args.filename_prefix,
            &args.output,
        )
        .unwrap();
        assert_eq!(parsed.split_by, d.split_by);
        assert_eq!(parsed.include_conductor, d.include_conductor);
        assert_eq!(parsed.output_format, d.output_format);
        assert_eq!(parsed.skip_empty, d.skip_empty);
        assert_eq!(parsed.select, d.select);
        assert_eq!(parsed.filename_prefix, d.filename_prefix);
        assert_eq!(parsed.output, d.output);
        assert_eq!(args.encoding, "auto");
    }
}
