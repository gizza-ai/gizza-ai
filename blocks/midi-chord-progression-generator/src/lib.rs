//! gizza-ai/midi-chord-progression-generator — turn chord symbols into a
//! downloadable Standard MIDI File. Pure compute, binary output, so `handle()`
//! builds a base64 download envelope rather than using `run_skill`.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
use gizza_ai_block_utils::{Envelope, ForUi, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_midi_chord_progression_generator_core::{convert, Options};
use serde::Deserialize;
use wafer_sdk::*;

const MIDI_MIME: &str = "audio/midi";
const MIDI_FILENAME: &str = "chord-progression.mid";

#[derive(Deserialize, Debug)]
#[serde(default)]
struct Args {
    progression: String,
    tempo: f64,
    beats_per_chord: f64,
    beats_per_bar: u32,
    octave: i32,
    voicing: String,
    inversion: String,
    pattern: String,
    arp_note: String,
    note_length: f64,
    add_bass: bool,
    transpose: i32,
    velocity: u8,
    instrument: String,
}

impl Default for Args {
    fn default() -> Self {
        let o = Options::default();
        Args {
            progression: String::new(),
            tempo: o.tempo,
            beats_per_chord: o.beats_per_chord,
            beats_per_bar: o.beats_per_bar,
            octave: o.octave,
            voicing: o.voicing,
            inversion: o.inversion,
            pattern: o.pattern,
            arp_note: o.arp_note,
            note_length: o.note_length,
            add_bass: o.add_bass,
            transpose: o.transpose,
            velocity: o.velocity,
            instrument: o.instrument,
        }
    }
}

impl From<&Args> for Options {
    fn from(a: &Args) -> Options {
        Options {
            tempo: a.tempo,
            beats_per_chord: a.beats_per_chord,
            beats_per_bar: a.beats_per_bar,
            octave: a.octave,
            voicing: a.voicing.clone(),
            inversion: a.inversion.clone(),
            pattern: a.pattern.clone(),
            arp_note: a.arp_note.clone(),
            note_length: a.note_length,
            add_bass: a.add_bass,
            transpose: a.transpose,
            velocity: a.velocity,
            instrument: a.instrument.clone(),
        }
    }
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("progression").required().describe("Chord symbols to render, separated by spaces, bars or new lines. Examples: `C G Am F`, `Cmaj7 | Dm7 G7 | Cmaj7`, `C/E:2 G:2 Am:4`. Use `R` or `rest` for a rest. A `:beats` suffix overrides the default length for that slot. Supports roots with sharps/flats, slash bass notes and common qualities such as m, 7, maj7, m7, dim, aug, sus2, sus4, 6, 9, 11 and 13. Max 64 KiB / 512 chord slots."))
        .param(Param::number("tempo").default(120.0).min(20.0).max(400.0).describe("Tempo in quarter-note beats per minute, written into the MIDI tempo event (default 120, range 20-400)."))
        .param(Param::number("beats_per_chord").default(4.0).min(0.25).max(64.0).describe("Default duration of each chord in beats unless the symbol has a `:beats` suffix. Default 4, so `C G Am F` is four bars in 4/4."))
        .param(Param::integer("beats_per_bar").default(4).min(1.0).max(16.0).describe("Time-signature numerator written to the MIDI file; denominator is quarter-note based. Default 4 (4/4)."))
        .param(Param::integer("octave").default(4).min(0.0).max(8.0).describe("Octave for the chord root before voicing and transposition. 4 puts middle C at MIDI note 60 (default 4)."))
        .param(Param::enumv("voicing", ["close", "drop-2", "drop-3", "spread"]).default("close").describe("How chord tones are arranged. `close` stacks them compactly, `drop-2` and `drop-3` move an upper voice down an octave, and `spread` opens the chord over a wider range."))
        .param(Param::enumv("inversion", ["root", "first", "second", "third", "smooth"]).default("root").describe("Bass/inversion choice. `root` keeps the root in the chord bass, first/second/third rotate chord tones up, and `smooth` chooses the nearest inversion to the previous chord."))
        .param(Param::enumv("pattern", ["block", "arpeggio-up", "arpeggio-down", "arpeggio-updown", "strum"]).default("block").describe("Playback pattern. `block` plays the whole chord at once, arpeggio modes step through notes, and `strum` staggers note starts while ending them together."))
        .param(Param::enumv("arp_note", ["whole", "half", "quarter", "eighth", "sixteenth", "thirty-second"]).default("eighth").describe("Note value for one arpeggio step. Ignored by block and strum patterns. Default eighth."))
        .param(Param::number("note_length").default(95.0).min(5.0).max(100.0).describe("Gate length as a percentage of the chord slot or arpeggio step, 5-100 (default 95). Lower values make more separation between notes."))
        .param(Param::boolean("add_bass").default(false).describe("Double the chord's bass note one octave below the voicing. Useful for piano or pad sketches that need a left-hand root."))
        .param(Param::integer("transpose").default(0).min(-24.0).max(24.0).describe("Transpose the generated notes after voicing, in semitones from -24 to +24 (default 0)."))
        .param(Param::integer("velocity").default(96).min(1.0).max(127.0).describe("MIDI note-on velocity (loudness), 1-127 (default 96)."))
        .param(Param::enumv("instrument", ["acoustic-grand-piano", "bright-acoustic-piano", "electric-piano", "harpsichord", "vibraphone", "drawbar-organ", "church-organ", "accordion", "acoustic-guitar-nylon", "acoustic-guitar-steel", "electric-guitar-clean", "acoustic-bass", "string-ensemble", "choir-aahs", "synth-pad-warm", "synth-lead-square"]).default("acoustic-grand-piano").describe("General MIDI instrument program written into the file (default acoustic-grand-piano). Your DAW can replace it later."))
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/midi-chord-progression-generator",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Generate a downloadable MIDI file from chord symbols",
    skill(
        description = "Convert a chord-symbol progression into a Standard MIDI File (.mid) with voiced chords and timing. `progression` accepts symbols such as `C G Am F`, `Cmaj7 | Dm7 G7 | Cmaj7`, slash chords like `C/E`, rests (`R`/`rest`) and per-chord duration suffixes such as `C:2`. The file includes tempo, time signature, General MIDI instrument, velocity and format-0 note events. Voicing controls include close, drop-2, drop-3, spread, root/first/second/third/smooth inversion, optional bass doubling and transposition. Patterns include block chords, up/down/up-down arpeggios and strums. Returns a base64 download envelope for a `.mid` file plus a summary for chat/CLI. Pure Rust/WASM; nothing is uploaded.",
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
    let args: Args = serde_json::from_slice(&body).map_err(|e| {
        SkillError::InvalidArgs(format!(
            "invalid midi-chord-progression-generator args: {e}"
        ))
    })?;
    run_args(&args).map_err(SkillError::InvalidArgs)
}

#[cfg(target_arch = "wasm32")]
fn run_args(args: &Args) -> Result<Vec<u8>, String> {
    let opts = Options::from(args);
    let out = convert(&args.progression, &opts)?;
    let env = Envelope {
        for_llm: out.summary(),
        for_ui: ForUi {
            data_url: format!("data:{MIDI_MIME};base64,{}", B64.encode(&out.midi)),
            mime: MIDI_MIME.to_string(),
            filename: MIDI_FILENAME.to_string(),
        },
    };
    serde_json::to_vec(&env).map_err(|e| format!("serialize envelope: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn descriptor_has_required_progression_and_enums() {
        let schema: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(schema["required"], serde_json::json!(["progression"]));
        assert!(schema["properties"]["voicing"]["enum"]
            .as_array()
            .unwrap()
            .iter()
            .any(|v| v == "drop-2"));
        assert!(schema["properties"]["pattern"]["enum"]
            .as_array()
            .unwrap()
            .iter()
            .any(|v| v == "strum"));
        assert_eq!(schema["properties"]["tempo"]["minimum"], 20.0);
    }

    #[test]
    fn args_defaults_match_core_options() {
        let args: Args = serde_json::from_str(r#"{"progression":"C G"}"#).unwrap();
        let opts = Options::default();
        assert_eq!(args.tempo, opts.tempo);
        assert_eq!(args.beats_per_chord, opts.beats_per_chord);
        assert_eq!(args.beats_per_bar, opts.beats_per_bar);
        assert_eq!(args.octave, opts.octave);
        assert_eq!(args.voicing, opts.voicing);
        assert_eq!(args.inversion, opts.inversion);
        assert_eq!(args.pattern, opts.pattern);
        assert_eq!(args.arp_note, opts.arp_note);
        assert_eq!(args.note_length, opts.note_length);
        assert_eq!(args.add_bass, opts.add_bass);
        assert_eq!(args.transpose, opts.transpose);
        assert_eq!(args.velocity, opts.velocity);
        assert_eq!(args.instrument, opts.instrument);
    }
}
