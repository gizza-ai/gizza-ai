//! gizza-ai/chord-progression-generator — chat skill block on the shared tool
//! abstraction. Generate a chord progression in any key, mode and style, with
//! Roman-numeral analysis, spelled chord tones and a Standard MIDI File.
//!
//! The chat schema is single-sourced from descriptor() (which also drives the
//! CLI and, via manifest.json, the page form); handle() delegates to
//! block_utils::run_skill, which hands the parsed Args to the shared core.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_chord_progression_generator_core as core;
use serde::Deserialize;
use wafer_sdk::*;

// Numeric bounds the core enforces. The enum vocabularies and the three
// count caps are imported from the core so they cannot drift; `tempo` and
// `octave` are range literals in the core, so their bounds are mirrored here
// and pinned by `numeric_bounds_match_what_the_core_accepts`.
const TEMPO_MIN: f64 = 40.0;
const TEMPO_MAX: f64 = 300.0;
const OCTAVE_MIN: i32 = 1;
const OCTAVE_MAX: i32 = 7;

const KEY_DESC: &str = "Tonic the progression is built on. Both spellings of every black key are offered: `Db` and `C#` sound the same but are written differently, and a flat key spells its chords with flats. Default C.";
const MODE_DESC: &str = "Scale the chords are drawn from — the seven church modes plus harmonic and melodic minor. `major` and `minor` cover most songs; the others change which degrees come out major, minor or diminished. Default major.";
const STYLE_DESC: &str = "Progression preset. Each style holds a handful of characteristic progressions; pick between them with `variation`. `random` instead walks the scale to build a fresh in-key progression. Default pop.";
const VARIATION_DESC: &str = "Which progression of the style to use, 1-99. Change it to re-roll: the same number always returns the same progression, so a result stays reproducible and shareable. Default 1.";
const SEVENTHS_DESC: &str = "Chord thickness. `auto` follows the style (jazz, lofi, R&B, hip-hop and ballad get sevenths; pop and rock stay triads), `triads` forces plain three-note chords, `sevenths` adds a seventh to every chord and `extended` adds ninths where the mode allows. Default auto.";
const BORROWED_DESC: &str = "Modal interchange. `none` keeps every chord inside the mode, `light` keeps the chromatic chords a style already uses (such as the bVII in rock and metal), and `rich` also recolours diatonic chords with borrowed ones like iv, bIII, bVI and secondary dominants. Default none.";
const CHORDS_DESC: &str = "How many chords the progression has, 1-32, or 0 for the preset's own length — 0 keeps a 12-bar blues at 12 bars. A larger value cycles the preset. Default 0.";
const TEMPO_DESC: &str = "Tempo in BPM written into the MIDI file, 40-300. It changes how fast the file plays, not which chords are chosen. Default 100.";
const INSTRUMENT_DESC: &str = "General MIDI instrument program written into the MIDI file, from acoustic-grand-piano to synth-pad-warm. Any DAW can swap it afterwards. Default acoustic-grand-piano.";
const PATTERN_DESC: &str = "How each chord is played in the MIDI file. `block` sounds every note together, the arpeggio modes step through the chord tones up, down or up-and-down, and `strum` staggers the note starts. Default block.";
const VOICE_LEADING_DESC: &str = "Voice-lead the MIDI file by picking the inversion of each chord nearest to the previous one, instead of restacking every chord in root position. Affects the file only, never the chord names. Default true.";
const REPEATS_DESC: &str = "How many times the progression is written into the MIDI file, 1-8, for a longer loop to work over. The printed analysis always shows a single pass. Default 1.";
const OCTAVE_DESC: &str = "Octave of the tonic before voicing, 1-7. 4 puts middle C at MIDI note 60; lower values move the whole file down. Default 4.";
const OUTPUT_DESC: &str = "Shape of the returned text. `text` is the full report (key, Roman numerals, chord symbols, spelled notes and file stats), `chords` and `roman` are one-line summaries, `csv` returns `bar,roman,chord,notes` rows, and `midi-base64` returns the Standard MIDI File itself as base64. Default text.";

/// Chat/CLI arguments. Every field is optional: `#[serde(default)]` plus the
/// `Default` impl below fall back to the core's own defaults, so a bare call
/// with no arguments still produces the classic C-major pop loop.
#[derive(Deserialize, Debug)]
#[serde(default)]
struct Args {
    key: String,
    mode: String,
    style: String,
    variation: i32,
    sevenths: String,
    borrowed: String,
    chords: i32,
    tempo: f64,
    instrument: String,
    pattern: String,
    voice_leading: bool,
    repeats: i32,
    octave: i32,
    output: String,
}

impl Default for Args {
    fn default() -> Self {
        let o = core::Options::default();
        Args {
            key: o.key,
            mode: o.mode,
            style: o.style,
            variation: o.variation,
            sevenths: o.sevenths,
            borrowed: o.borrowed,
            chords: o.chords,
            tempo: o.tempo,
            instrument: o.instrument,
            pattern: o.pattern,
            voice_leading: o.voice_leading,
            repeats: o.repeats,
            octave: o.octave,
            output: o.output,
        }
    }
}

impl From<&Args> for core::Options {
    fn from(a: &Args) -> core::Options {
        core::Options {
            key: a.key.clone(),
            mode: a.mode.clone(),
            style: a.style.clone(),
            variation: a.variation,
            sevenths: a.sevenths.clone(),
            borrowed: a.borrowed.clone(),
            chords: a.chords,
            tempo: a.tempo,
            instrument: a.instrument.clone(),
            pattern: a.pattern.clone(),
            voice_leading: a.voice_leading,
            repeats: a.repeats,
            octave: a.octave,
            output: a.output.clone(),
        }
    }
}

/// Single source for the chat schema, the CLI flags and (after
/// `scripts/sync-tool-manifest.py`) the page form's controls. The enum
/// vocabularies come straight from the core constants, so an option the core
/// learns or forgets can never drift out of the advertised dropdowns.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::enumv("key", core::KEYS)
                .default(core::Options::default().key)
                .describe(KEY_DESC),
        )
        .param(
            Param::enumv("mode", core::MODES)
                .default(core::Options::default().mode)
                .describe(MODE_DESC),
        )
        .param(
            Param::enumv("style", core::STYLES)
                .default(core::Options::default().style)
                .describe(STYLE_DESC),
        )
        .param(
            Param::integer("variation")
                .min(1.0)
                .max(core::MAX_VARIATION as f64)
                .default(core::Options::default().variation)
                .describe(VARIATION_DESC),
        )
        .param(
            Param::enumv("sevenths", core::SEVENTHS)
                .default(core::Options::default().sevenths)
                .describe(SEVENTHS_DESC),
        )
        .param(
            Param::enumv("borrowed", core::BORROWED)
                .default(core::Options::default().borrowed)
                .describe(BORROWED_DESC),
        )
        .param(
            Param::integer("chords")
                .min(0.0)
                .max(core::MAX_CHORDS as f64)
                .default(core::Options::default().chords)
                .describe(CHORDS_DESC),
        )
        .param(
            Param::number("tempo")
                .min(TEMPO_MIN)
                .max(TEMPO_MAX)
                .default(core::Options::default().tempo)
                .describe(TEMPO_DESC),
        )
        .param(
            Param::enumv("instrument", core::INSTRUMENTS)
                .default(core::Options::default().instrument)
                .describe(INSTRUMENT_DESC),
        )
        .param(
            Param::enumv("pattern", core::PATTERNS)
                .default(core::Options::default().pattern)
                .describe(PATTERN_DESC),
        )
        .param(
            Param::boolean("voice_leading")
                .default(core::Options::default().voice_leading)
                .describe(VOICE_LEADING_DESC),
        )
        .param(
            Param::integer("repeats")
                .min(1.0)
                .max(core::MAX_REPEATS as f64)
                .default(core::Options::default().repeats)
                .describe(REPEATS_DESC),
        )
        .param(
            Param::integer("octave")
                .min(OCTAVE_MIN as f64)
                .max(OCTAVE_MAX as f64)
                .default(core::Options::default().octave)
                .describe(OCTAVE_DESC),
        )
        .param(
            Param::enumv("output", core::OUTPUTS)
                .default(core::Options::default().output)
                .describe(OUTPUT_DESC),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

/// Build the core options from the parsed arguments and render the result. The
/// core validates every value and reports what it expected instead.
fn run_args(args: &Args) -> Result<String, String> {
    core::run(&core::Options::from(args))
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/chord-progression-generator",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Generate a chord progression in any key, mode and style",
    skill(
        description = "Generate a chord progression in any key, mode and style, with Roman-numeral analysis, the spelled notes of every chord and a Standard MIDI File. `key` offers 17 tonic spellings, `mode` covers major, minor, dorian, phrygian, lydian, mixolydian, locrian, harmonic-minor and melodic-minor, and `style` picks a preset (pop, rock, folk, country, ballad, worship, edm, hip-hop, lofi, rnb, jazz, blues, reggae, metal, cinematic) or `random` for a fresh in-key walk. `variation` re-rolls within a style; `chords` sets the length, with 0 keeping the preset's own (a 12-bar blues stays 12 bars). `sevenths` chooses triads, sevenths or extended chords and `borrowed` adds modal interchange such as iv, bIII, bVI and bVII. `tempo`, `instrument`, `pattern`, `octave`, `repeats` and `voice_leading` shape the rendered MIDI. `output` returns the full report, a bare chord line, a Roman-numeral line, CSV rows, or the .mid file as base64. Generation is deterministic: the same options always give the same progression and the same MIDI bytes. Pure Rust/WASM; nothing is uploaded.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        // run_skill wraps the returned value in { "result": ... }. The MIDI
        // file is reachable as text through `output=midi-base64`, so this stays
        // a plain textual skill rather than a media envelope.
        match run_skill(&body, "chord-progression-generator", |a: Args| {
            run_args(&a).map_err(SkillError::InvalidArgs)
        }) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn schema() -> serde_json::Value {
        serde_json::from_str(&schema_json()).unwrap()
    }

    fn variants(param: &str) -> Vec<String> {
        schema()["properties"][param]["enum"]
            .as_array()
            .unwrap_or_else(|| panic!("{param} has no enum"))
            .iter()
            .map(|v| v.as_str().unwrap().to_string())
            .collect()
    }

    #[test]
    fn dump_schema() {
        println!("SCHEMA_BEGIN{}SCHEMA_END", schema_json());
    }

    /// Every advertised parameter needs prose a chat model and the page form can
    /// both use — an undescribed param is an unusable dropdown.
    #[test]
    fn every_param_is_described() {
        let s = schema();
        let props = s["properties"].as_object().expect("object schema");
        assert_eq!(props.len(), 14, "parameter count changed");
        for (name, spec) in props {
            let desc = spec["description"].as_str().unwrap_or("");
            assert!(desc.len() > 40, "param {name} needs a real description");
            assert!(
                spec.get("default").is_some(),
                "param {name} has no default, but nothing is required"
            );
        }
        // Nothing is required: a bare call still returns a usable progression.
        assert!(
            s.get("required").is_none(),
            "no parameter should be required"
        );
        assert_eq!(s["additionalProperties"], serde_json::json!(false));
    }

    /// The descriptor's declared defaults are what chat/the CLI leave out, so
    /// serde's `#[serde(default)]` fallbacks must agree with them.
    #[test]
    fn args_defaults_match_the_descriptor_and_the_core() {
        let a: Args = serde_json::from_str("{}").unwrap();
        let props = schema()["properties"].clone();
        assert_eq!(a.key, props["key"]["default"]);
        assert_eq!(a.mode, props["mode"]["default"]);
        assert_eq!(a.style, props["style"]["default"]);
        assert_eq!(i64::from(a.variation), props["variation"]["default"]);
        assert_eq!(a.sevenths, props["sevenths"]["default"]);
        assert_eq!(a.borrowed, props["borrowed"]["default"]);
        assert_eq!(i64::from(a.chords), props["chords"]["default"]);
        assert_eq!(a.tempo, props["tempo"]["default"].as_f64().unwrap());
        assert_eq!(a.instrument, props["instrument"]["default"]);
        assert_eq!(a.pattern, props["pattern"]["default"]);
        assert_eq!(a.voice_leading, props["voice_leading"]["default"]);
        assert_eq!(i64::from(a.repeats), props["repeats"]["default"]);
        assert_eq!(i64::from(a.octave), props["octave"]["default"]);
        assert_eq!(a.output, props["output"]["default"]);
        assert_eq!(core::Options::from(&a), core::Options::default());
    }

    /// A call with no arguments at all must reach the core and produce the
    /// documented headline, so a default drifting out of the core's accepted
    /// set is caught here rather than in chat.
    #[test]
    fn defaulted_args_run_through_the_core() {
        let a: Args = serde_json::from_str("{}").unwrap();
        let out = run_args(&a).unwrap();
        assert!(
            out.contains("Key: C major | Style: pop (variation 1)"),
            "{out}"
        );
        assert!(out.contains("Roman:  I V vi IV"), "{out}");
        assert!(out.contains("Chords: C G Am F"), "{out}");
    }

    /// Arguments really flow through to the core rather than being ignored.
    #[test]
    fn args_reach_the_core() {
        let a: Args = serde_json::from_str(
            r#"{"key":"Eb","mode":"minor","style":"jazz","variation":2,
                "sevenths":"sevenths","chords":4,"output":"chords"}"#,
        )
        .unwrap();
        assert_eq!(run_args(&a).unwrap(), "Ebm7 Cbmaj7 Fm7b5 Bbm7");
        let a: Args = serde_json::from_str(r#"{"output":"roman"}"#).unwrap();
        assert_eq!(run_args(&a).unwrap(), "I V vi IV");
        let a: Args = serde_json::from_str(r#"{"key":"Q"}"#).unwrap();
        assert!(run_args(&a).unwrap_err().contains("unknown key 'Q'"));
    }

    /// Every enum variant the descriptor advertises must be one the core
    /// accepts — an advertised-but-rejected option is a broken dropdown.
    #[test]
    fn every_advertised_enum_variant_is_accepted_by_the_core() {
        for (param, field) in [
            ("key", "key"),
            ("mode", "mode"),
            ("style", "style"),
            ("sevenths", "sevenths"),
            ("borrowed", "borrowed"),
            ("instrument", "instrument"),
            ("pattern", "pattern"),
            ("output", "output"),
        ] {
            for v in variants(param) {
                let json = format!(r#"{{"{field}":"{v}"}}"#);
                let a: Args = serde_json::from_str(&json).unwrap();
                run_args(&a).unwrap_or_else(|e| panic!("{param}={v}: {e}"));
            }
        }
    }

    /// The advertised numeric bounds must be exactly the ones the core enforces:
    /// a min/max the core rejects is a slider that fails at its own end stop.
    #[test]
    fn numeric_bounds_match_what_the_core_accepts() {
        let s = schema();
        let at = |field: &str, v: f64| -> Result<String, String> {
            let a: Args = serde_json::from_str(&format!(r#"{{"{field}":{v}}}"#)).unwrap();
            run_args(&a)
        };
        for field in ["variation", "chords", "tempo", "repeats", "octave"] {
            let lo = s["properties"][field]["minimum"].as_f64().unwrap();
            let hi = s["properties"][field]["maximum"].as_f64().unwrap();
            at(field, lo).unwrap_or_else(|e| panic!("{field}={lo} rejected: {e}"));
            at(field, hi).unwrap_or_else(|e| panic!("{field}={hi} rejected: {e}"));
            assert!(
                at(field, lo - 1.0).is_err(),
                "{field} accepts below its minimum"
            );
            assert!(
                at(field, hi + 1.0).is_err(),
                "{field} accepts above its maximum"
            );
        }
        assert_eq!(s["properties"]["variation"]["maximum"], core::MAX_VARIATION);
        assert_eq!(s["properties"]["chords"]["maximum"], core::MAX_CHORDS);
        assert_eq!(s["properties"]["repeats"]["maximum"], core::MAX_REPEATS);
    }

    /// Drift guard: the chat/CLI/page schema is generated from `descriptor()`,
    /// so any change to a param name, type, enum, bound or default must be
    /// mirrored here.
    #[test]
    fn schema_matches_the_authored_contract() {
        let authored = serde_json::json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "key": {
                    "type": "string",
                    "enum": ["C", "C#", "Db", "D", "D#", "Eb", "E", "F", "F#", "Gb",
                             "G", "G#", "Ab", "A", "A#", "Bb", "B"],
                    "default": "C",
                    "description": KEY_DESC
                },
                "mode": {
                    "type": "string",
                    "enum": ["major", "minor", "dorian", "phrygian", "lydian",
                             "mixolydian", "locrian", "harmonic-minor", "melodic-minor"],
                    "default": "major",
                    "description": MODE_DESC
                },
                "style": {
                    "type": "string",
                    "enum": ["pop", "rock", "folk", "country", "ballad", "worship", "edm",
                             "hip-hop", "lofi", "rnb", "jazz", "blues", "reggae", "metal",
                             "cinematic", "random"],
                    "default": "pop",
                    "description": STYLE_DESC
                },
                "variation": {
                    "type": "integer", "minimum": 1, "maximum": 99,
                    "default": 1, "description": VARIATION_DESC
                },
                "sevenths": {
                    "type": "string",
                    "enum": ["auto", "triads", "sevenths", "extended"],
                    "default": "auto",
                    "description": SEVENTHS_DESC
                },
                "borrowed": {
                    "type": "string",
                    "enum": ["none", "light", "rich"],
                    "default": "none",
                    "description": BORROWED_DESC
                },
                "chords": {
                    "type": "integer", "minimum": 0, "maximum": 32,
                    "default": 0, "description": CHORDS_DESC
                },
                "tempo": {
                    "type": "number", "minimum": 40, "maximum": 300,
                    "default": 100.0, "description": TEMPO_DESC
                },
                "instrument": {
                    "type": "string",
                    "enum": ["acoustic-grand-piano", "bright-acoustic-piano", "electric-piano",
                             "harpsichord", "vibraphone", "drawbar-organ", "church-organ",
                             "accordion", "acoustic-guitar-nylon", "acoustic-guitar-steel",
                             "electric-guitar-clean", "acoustic-bass", "string-ensemble",
                             "choir-aahs", "synth-pad-warm", "synth-lead-square"],
                    "default": "acoustic-grand-piano",
                    "description": INSTRUMENT_DESC
                },
                "pattern": {
                    "type": "string",
                    "enum": ["block", "arpeggio-up", "arpeggio-down", "arpeggio-updown", "strum"],
                    "default": "block",
                    "description": PATTERN_DESC
                },
                "voice_leading": {
                    "type": "boolean", "default": true, "description": VOICE_LEADING_DESC
                },
                "repeats": {
                    "type": "integer", "minimum": 1, "maximum": 8,
                    "default": 1, "description": REPEATS_DESC
                },
                "octave": {
                    "type": "integer", "minimum": 1, "maximum": 7,
                    "default": 4, "description": OCTAVE_DESC
                },
                "output": {
                    "type": "string",
                    "enum": ["text", "chords", "roman", "csv", "midi-base64"],
                    "default": "text",
                    "description": OUTPUT_DESC
                }
            }
        });
        assert_eq!(schema(), authored);
    }

    /// The page/CLI-facing manifest is generated from this block's descriptor
    /// (scripts/sync-tool-manifest.py) — guard it against silent drift. Without
    /// this, an enum lost from the manifest renders a text box instead of a
    /// <select> on the tool page.
    #[test]
    fn manifest_tool_section_matches_the_live_descriptor() {
        let manifest: serde_json::Value =
            serde_json::from_str(include_str!("../manifest.json")).unwrap();
        assert_eq!(
            manifest["tool"]["parameters"],
            schema(),
            "manifest.json tool.parameters drifted from descriptor()"
        );
        assert_eq!(manifest["name"], "gizza-ai/chord-progression-generator");
        assert_eq!(manifest["summary"], MACRO_SUMMARY);
    }

    /// The summary shown in chat (the wafer_block macro), in manifest.json and
    /// in wafer.toml must stay identical — scripts/check-tool-hygiene.py fails
    /// the build when they diverge.
    const MACRO_SUMMARY: &str = "Generate a chord progression in any key, mode and style";

    #[test]
    fn summaries_agree_across_the_three_metadata_files() {
        assert!(
            include_str!("lib.rs").contains(&format!("summary = \"{MACRO_SUMMARY}\"")),
            "the wafer_block macro summary changed"
        );
        assert!(
            include_str!("../wafer.toml").contains(&format!("summary = \"{MACRO_SUMMARY}\"")),
            "wafer.toml summary drifted from the macro summary"
        );
    }
}
