//! gizza-ai/guitar-tab-to-midi — turn ASCII guitar/bass tablature into a real
//! Standard MIDI File. Pure compute, binary output, so `handle()` builds a
//! base64 download envelope like csv-to-xlsx rather than using `run_skill`.
//! The chat schema is single-sourced from `descriptor()` (which also drives the
//! CLI); the page calls the same core through `web/`.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
use gizza_ai_block_utils::{Envelope, ForUi, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_guitar_tab_to_midi_core::{convert, Options};
use serde::Deserialize;
use wafer_sdk::*;

/// Standard MIDI File MIME type.
const MIDI_MIME: &str = "audio/midi";
/// Download name for the produced file.
const MIDI_FILENAME: &str = "guitar-tab.mid";

#[derive(Deserialize, Debug)]
#[serde(default)]
struct Args {
    tab: String,
    tuning: String,
    custom_tuning: String,
    string_order: String,
    capo: i32,
    transpose: i32,
    tempo: f64,
    note_duration: String,
    timing: String,
    sustain: String,
    velocity: u8,
    instrument: String,
    muted_notes: bool,
}

impl Default for Args {
    fn default() -> Self {
        let o = Options::default();
        Args {
            tab: String::new(),
            tuning: o.tuning,
            custom_tuning: o.custom_tuning,
            string_order: o.string_order,
            capo: o.capo,
            transpose: o.transpose,
            tempo: o.tempo,
            note_duration: o.note_duration,
            timing: o.timing,
            sustain: o.sustain,
            velocity: o.velocity,
            instrument: o.instrument,
            muted_notes: o.muted_notes,
        }
    }
}

impl From<Args> for Options {
    fn from(a: Args) -> Options {
        Options {
            tuning: a.tuning,
            custom_tuning: a.custom_tuning,
            string_order: a.string_order,
            capo: a.capo,
            transpose: a.transpose,
            tempo: a.tempo,
            note_duration: a.note_duration,
            timing: a.timing,
            sustain: a.sustain,
            velocity: a.velocity,
            instrument: a.instrument,
            muted_notes: a.muted_notes,
        }
    }
}

/// Single source for the chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("tab")
                .required()
                .describe("The ASCII tablature, one line per string, e.g. `e|--0--3--|` … `E|--------|`. Fret numbers sound as notes (a run of digits is one fret, so `12` is the twelfth fret); `-` is a rest; a `|` column that spans every string is a bar line and takes no time; `h p b r s t / \\ ~ ^ * . ( )` are treated as articulation marks and skipped; `x`/`X` are muted strings (see `muted_notes`). Several staves separated by blank or prose lines are concatenated in reading order. At most 1 MiB of text and 20000 notes."),
        )
        .param(
            Param::enumv(
                "tuning",
                [
                    "auto",
                    "standard",
                    "drop-d",
                    "half-step-down",
                    "full-step-down",
                    "drop-c",
                    "open-g",
                    "open-d",
                    "dadgad",
                    "seven-string",
                    "eight-string",
                    "bass-standard",
                    "bass-5-string",
                    "ukulele",
                ],
            )
            .default("auto")
            .describe("Open-string tuning. 'auto' (default) picks from the number of string lines: 4 → bass-standard (E1 A1 D2 G2), 5 → bass-5-string, 6 → standard (E2 A2 D3 G3 B3 E4), 7 → seven-string, 8 → eight-string. Named presets: standard, drop-d, half-step-down, full-step-down, drop-c, open-g, open-d, dadgad, seven-string, eight-string, bass-standard, bass-5-string, ukulele (gCEA). The preset must have as many strings as the tab has lines. Ignored when `custom_tuning` is set."),
        )
        .param(
            Param::string("custom_tuning")
                .default("")
                .describe("Explicit open-string pitches, overriding `tuning`. Comma- or space-separated, one per string line, BOTTOM tab line first, as scientific pitch names or MIDI numbers — e.g. `D2,A2,D3,G3,B3,E4` (drop D) or `38 45 50 55 59 64`. Empty (default) uses `tuning`."),
        )
        .param(
            Param::enumv("string_order", ["high-on-top", "low-on-top"])
                .default("high-on-top")
                .describe("Which string the FIRST line of the stave is. 'high-on-top' (default) is the usual layout — the top line is the thinnest/highest string (high E on a guitar). Use 'low-on-top' for tabs written the other way up, otherwise the file comes out pitch-inverted."),
        )
        .param(
            Param::integer("capo")
                .default(0)
                .min(0.0)
                .max(12.0)
                .describe("Capo position in frets, added to every string (default 0, max 12). A capo on fret 2 with standard tuning sounds F#2 B2 E3 A3 C#4 F#4."),
        )
        .param(
            Param::integer("transpose")
                .default(0)
                .min(-24.0)
                .max(24.0)
                .describe("Extra transposition in semitones, applied after the capo (default 0, range -24 to +24). Use -12 to drop an octave, +12 to raise one."),
        )
        .param(
            Param::number("tempo")
                .default(120.0)
                .min(20.0)
                .max(400.0)
                .describe("Tempo in quarter-note beats per minute, written as a MIDI tempo event (default 120, range 20-400)."),
        )
        .param(
            Param::enumv(
                "note_duration",
                ["whole", "half", "quarter", "eighth", "sixteenth", "thirty-second"],
            )
            .default("eighth")
            .describe("Musical length of ONE tab column, which sets the whole rhythm (default 'eighth'). With eighth notes at 120 BPM each column lasts 0.25 s. Use 'sixteenth' for busy tabs and 'quarter' for sparse ones."),
        )
        .param(
            Param::enumv("timing", ["columns", "events"])
                .default("columns")
                .describe("How time advances. 'columns' (default) gives every character column one step, so the dash spacing in the tab IS the rhythm. 'events' only counts columns that contain a note, spacing the onsets evenly — use it when the tab's spacing is decorative rather than rhythmic."),
        )
        .param(
            Param::enumv("sustain", ["until-next", "step"])
                .default("until-next")
                .describe("How long each note rings. 'until-next' (default) holds a string until its next note on that string (or the end of the tab), like a real guitar. 'step' makes every note exactly one column long, giving a detached, sequencer-like feel."),
        )
        .param(
            Param::integer("velocity")
                .default(96)
                .min(1.0)
                .max(127.0)
                .describe("MIDI note-on velocity (loudness) for every note, 1-127 (default 96)."),
        )
        .param(
            Param::enumv(
                "instrument",
                [
                    "acoustic-guitar-nylon",
                    "acoustic-guitar-steel",
                    "electric-guitar-jazz",
                    "electric-guitar-clean",
                    "electric-guitar-muted",
                    "overdriven-guitar",
                    "distortion-guitar",
                    "guitar-harmonics",
                    "acoustic-bass",
                    "electric-bass-finger",
                    "electric-bass-pick",
                    "fretless-bass",
                    "acoustic-grand-piano",
                ],
            )
            .default("acoustic-guitar-steel")
            .describe("General MIDI instrument written as a program-change event (default 'acoustic-guitar-steel', GM program 26). Your player or DAW can always override it."),
        )
        .param(
            Param::boolean("muted_notes")
                .default(false)
                .describe("Render `x`/`X` dead/muted strings as short muted notes on the open string instead of skipping them (default false: mutes are silent)."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct GuitarTabToMidi;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/guitar-tab-to-midi",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Convert ASCII guitar or bass tablature into a playable Standard MIDI File",
    skill(
        description = "Convert ASCII guitar/bass tablature into a real Standard MIDI File (.mid) and return it as a download. `tab` is the pasted tablature, one line per string; a run of digits is a fret number (so `12` is one note, not two), `-` is a rest, a `|` column spanning every string is a bar line that takes no time, `h p b r s t / \\ ~ ^ * . ( )` are skipped as articulation marks, and `x`/`X` are muted strings. Every string with a fret in the same column sounds together as a chord, and multiple staves are concatenated in reading order. Pitch = open string + capo + fret + transpose, with the open strings coming from a `tuning` preset (auto-detected from the number of string lines: 4 → bass, 6 → guitar, 7/8 → extended range) or an explicit `custom_tuning` list, bottom tab line first. `string_order` handles tabs written low-string-on-top. `note_duration` sets how long one column lasts and `tempo` the BPM; `timing` picks whether every column or only note columns advance time; `sustain` picks whether a note rings until the string's next note or stops after one column. `velocity` and a General MIDI `instrument` are written into the file. Errors name the offending line and column. Pure compute — nothing is fetched, uploaded or stored.",
        parameters = schema_json()
    ),
)]
impl GuitarTabToMidi {
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
        .map_err(|e| SkillError::InvalidArgs(format!("invalid guitar-tab-to-midi args: {e}")))?;
    let tab = args.tab.clone();
    let opts: Options = args.into();
    let out = convert(&tab, &opts).map_err(SkillError::InvalidArgs)?;

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
                    "tab": { "type": "string", "description": "The ASCII tablature, one line per string, e.g. `e|--0--3--|` … `E|--------|`. Fret numbers sound as notes (a run of digits is one fret, so `12` is the twelfth fret); `-` is a rest; a `|` column that spans every string is a bar line and takes no time; `h p b r s t / \\ ~ ^ * . ( )` are treated as articulation marks and skipped; `x`/`X` are muted strings (see `muted_notes`). Several staves separated by blank or prose lines are concatenated in reading order. At most 1 MiB of text and 20000 notes." },
                    "tuning": { "type": "string", "enum": ["auto", "standard", "drop-d", "half-step-down", "full-step-down", "drop-c", "open-g", "open-d", "dadgad", "seven-string", "eight-string", "bass-standard", "bass-5-string", "ukulele"], "default": "auto", "description": "Open-string tuning. 'auto' (default) picks from the number of string lines: 4 → bass-standard (E1 A1 D2 G2), 5 → bass-5-string, 6 → standard (E2 A2 D3 G3 B3 E4), 7 → seven-string, 8 → eight-string. Named presets: standard, drop-d, half-step-down, full-step-down, drop-c, open-g, open-d, dadgad, seven-string, eight-string, bass-standard, bass-5-string, ukulele (gCEA). The preset must have as many strings as the tab has lines. Ignored when `custom_tuning` is set." },
                    "custom_tuning": { "type": "string", "default": "", "description": "Explicit open-string pitches, overriding `tuning`. Comma- or space-separated, one per string line, BOTTOM tab line first, as scientific pitch names or MIDI numbers — e.g. `D2,A2,D3,G3,B3,E4` (drop D) or `38 45 50 55 59 64`. Empty (default) uses `tuning`." },
                    "string_order": { "type": "string", "enum": ["high-on-top", "low-on-top"], "default": "high-on-top", "description": "Which string the FIRST line of the stave is. 'high-on-top' (default) is the usual layout — the top line is the thinnest/highest string (high E on a guitar). Use 'low-on-top' for tabs written the other way up, otherwise the file comes out pitch-inverted." },
                    "capo": { "type": "integer", "default": 0, "minimum": 0, "maximum": 12, "description": "Capo position in frets, added to every string (default 0, max 12). A capo on fret 2 with standard tuning sounds F#2 B2 E3 A3 C#4 F#4." },
                    "transpose": { "type": "integer", "default": 0, "minimum": -24, "maximum": 24, "description": "Extra transposition in semitones, applied after the capo (default 0, range -24 to +24). Use -12 to drop an octave, +12 to raise one." },
                    "tempo": { "type": "number", "default": 120.0, "minimum": 20, "maximum": 400, "description": "Tempo in quarter-note beats per minute, written as a MIDI tempo event (default 120, range 20-400)." },
                    "note_duration": { "type": "string", "enum": ["whole", "half", "quarter", "eighth", "sixteenth", "thirty-second"], "default": "eighth", "description": "Musical length of ONE tab column, which sets the whole rhythm (default 'eighth'). With eighth notes at 120 BPM each column lasts 0.25 s. Use 'sixteenth' for busy tabs and 'quarter' for sparse ones." },
                    "timing": { "type": "string", "enum": ["columns", "events"], "default": "columns", "description": "How time advances. 'columns' (default) gives every character column one step, so the dash spacing in the tab IS the rhythm. 'events' only counts columns that contain a note, spacing the onsets evenly — use it when the tab's spacing is decorative rather than rhythmic." },
                    "sustain": { "type": "string", "enum": ["until-next", "step"], "default": "until-next", "description": "How long each note rings. 'until-next' (default) holds a string until its next note on that string (or the end of the tab), like a real guitar. 'step' makes every note exactly one column long, giving a detached, sequencer-like feel." },
                    "velocity": { "type": "integer", "default": 96, "minimum": 1, "maximum": 127, "description": "MIDI note-on velocity (loudness) for every note, 1-127 (default 96)." },
                    "instrument": { "type": "string", "enum": ["acoustic-guitar-nylon", "acoustic-guitar-steel", "electric-guitar-jazz", "electric-guitar-clean", "electric-guitar-muted", "overdriven-guitar", "distortion-guitar", "guitar-harmonics", "acoustic-bass", "electric-bass-finger", "electric-bass-pick", "fretless-bass", "acoustic-grand-piano"], "default": "acoustic-guitar-steel", "description": "General MIDI instrument written as a program-change event (default 'acoustic-guitar-steel', GM program 26). Your player or DAW can always override it." },
                    "muted_notes": { "type": "boolean", "default": false, "description": "Render `x`/`X` dead/muted strings as short muted notes on the open string instead of skipping them (default false: mutes are silent)." }
                },
                "required": ["tab"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }

    /// The `Args` defaults the chat/CLI surface applies must be the SAME
    /// defaults the core documents, so an omitted param behaves identically on
    /// every surface.
    #[test]
    fn args_defaults_match_core_option_defaults() {
        let args: Args = serde_json::from_str(r#"{"tab":"x"}"#).unwrap();
        let o = Options::default();
        assert_eq!(args.tuning, o.tuning);
        assert_eq!(args.string_order, o.string_order);
        assert_eq!(args.capo, o.capo);
        assert_eq!(args.transpose, o.transpose);
        assert_eq!(args.tempo, o.tempo);
        assert_eq!(args.note_duration, o.note_duration);
        assert_eq!(args.timing, o.timing);
        assert_eq!(args.sustain, o.sustain);
        assert_eq!(args.velocity, o.velocity);
        assert_eq!(args.instrument, o.instrument);
        assert_eq!(args.muted_notes, o.muted_notes);
    }
}
