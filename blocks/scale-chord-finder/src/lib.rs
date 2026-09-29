//! gizza-ai/scale-chord-finder — chat skill block on the shared tool
//! abstraction. Two directions of one lookup: name the scales and modes that
//! contain a set of notes, or spell a scale and its diatonic chords.
//!
//! The chat schema is single-sourced from descriptor() (which also drives the
//! CLI and, via manifest.json, the page form); handle() delegates to
//! block_utils::run_skill, which hands the parsed Args to the shared core.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_scale_chord_finder_core as core;
use serde::Deserialize;
use wafer_sdk::*;

const ACTION_DESC: &str = "Which direction of the lookup to run. `find` takes the notes you played and reports the scales and modes that fit them; `list` spells one named scale and its diatonic chords. `auto` picks `find` when `notes` is given and `list` when it is blank. Default auto.";
const NOTES_DESC: &str = "Notes to search for, as letters A-G with optional accidentals, separated by spaces or commas — for example `C E G B`, `F#, A, C#` or `Db Eb Gb Ab Bb`. Octave numbers (`C4`) are accepted and ignored, duplicates collapse to one pitch class, and up to 24 tokens are read. Only used by `find`.";
const ROOT_DESC: &str = "Tonic filter for `find`. `any` searches all twelve roots; naming a root keeps only the scales built on it and also fixes the spelling, so `Eb` reports Eb scales rather than their D# twins. Default any.";
const KEY_DESC: &str = "Tonic the scale is built on for `list`. Both spellings of every black key are offered: `Db` and `C#` sound the same but spell their notes and chords differently. Default C.";
const SCALE_DESC: &str = "Scale or mode to spell for `list` — the seven major modes, the pentatonics and blues scales, the modes of harmonic and melodic minor, harmonic major, the symmetric, bebop and Japanese scales, and chromatic. Default major.";
const FIT_DESC: &str = "How closely a scale has to fit the searched notes. `contains` keeps every scale holding all of them, `exact` keeps only scales whose note set is exactly yours (same size, no extras), and `near` also allows scales missing one note. Default contains.";
const SPELLING_DESC: &str = "Accidental preference for the printed names. `auto` spells each scale the way notation would (flat keys get flats, sharp keys get sharps), while `sharps` and `flats` force one accidental throughout. Default auto.";
const INCLUDE_CHORDS_DESC: &str = "Include the diatonic chords built on each scale degree, with their Roman numerals. Turn it off for a bare note-and-degree report. Default true.";
const INCLUDE_MODES_DESC: &str = "Include scales that share the same notes. In `list` this prints the parent scale's other modes (`Same notes as: D major`); in `find` it keeps every rotation of a matching note set instead of collapsing them to one row. Default true.";
const CHORD_TYPE_DESC: &str = "Which diatonic chords to build. `triads` gives three-note chords, `sevenths` gives four-note chords, and `both` prints the triad and seventh rows together. Ignored when `include_chords` is false. Default triads.";
const MAX_RESULTS_DESC: &str = "How many matching scales `find` reports, 1-50. The report always states how many matches were found in total, so a low value truncates the list rather than the search. Default 12.";
const OUTPUT_DESC: &str = "Shape of the returned text. `text` is the full report, `names` is just the note names of the scale (or the top match), `csv` returns one row per degree (or per match), and `json` returns the whole result as a JSON object. Default text.";

/// Chat/CLI arguments. Every field is optional: `#[serde(default)]` plus the
/// `Default` impl below fall back to the core's own defaults, so a bare call
/// with no arguments still spells C major and its triads.
#[derive(Deserialize, Debug)]
#[serde(default)]
struct Args {
    action: String,
    notes: String,
    root: String,
    key: String,
    scale: String,
    fit: String,
    spelling: String,
    include_chords: bool,
    include_modes: bool,
    chord_type: String,
    max_results: i32,
    output: String,
}

impl Default for Args {
    fn default() -> Self {
        let o = core::Options::default();
        Args {
            action: o.action,
            notes: o.notes,
            root: o.root,
            key: o.key,
            scale: o.scale,
            fit: o.fit,
            spelling: o.spelling,
            include_chords: o.include_chords,
            include_modes: o.include_modes,
            chord_type: o.chord_type,
            max_results: o.max_results,
            output: o.output,
        }
    }
}

impl From<&Args> for core::Options {
    fn from(a: &Args) -> core::Options {
        core::Options {
            action: a.action.clone(),
            notes: a.notes.clone(),
            root: a.root.clone(),
            key: a.key.clone(),
            scale: a.scale.clone(),
            fit: a.fit.clone(),
            spelling: a.spelling.clone(),
            include_chords: a.include_chords,
            include_modes: a.include_modes,
            chord_type: a.chord_type.clone(),
            max_results: a.max_results,
            output: a.output.clone(),
        }
    }
}

/// Single source for the chat schema, the CLI flags and (after
/// `scripts/sync-tool-manifest.py`) the page form's controls. The enum
/// vocabularies come straight from the core constants, so a scale or option the
/// core learns or forgets can never drift out of the advertised dropdowns.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::enumv("action", core::ACTIONS)
                .default(core::Options::default().action)
                .describe(ACTION_DESC),
        )
        .param(
            Param::string("notes")
                .default(core::Options::default().notes)
                .describe(NOTES_DESC),
        )
        .param(
            Param::enumv("root", core::ROOTS)
                .default(core::Options::default().root)
                .describe(ROOT_DESC),
        )
        .param(
            Param::enumv("key", core::KEYS)
                .default(core::Options::default().key)
                .describe(KEY_DESC),
        )
        .param(
            Param::enumv("scale", core::SCALES)
                .default(core::Options::default().scale)
                .describe(SCALE_DESC),
        )
        .param(
            Param::enumv("fit", core::FITS)
                .default(core::Options::default().fit)
                .describe(FIT_DESC),
        )
        .param(
            Param::enumv("spelling", core::SPELLINGS)
                .default(core::Options::default().spelling)
                .describe(SPELLING_DESC),
        )
        .param(
            Param::boolean("include_chords")
                .default(core::Options::default().include_chords)
                .describe(INCLUDE_CHORDS_DESC),
        )
        .param(
            Param::boolean("include_modes")
                .default(core::Options::default().include_modes)
                .describe(INCLUDE_MODES_DESC),
        )
        .param(
            Param::enumv("chord_type", core::CHORD_TYPES)
                .default(core::Options::default().chord_type)
                .describe(CHORD_TYPE_DESC),
        )
        .param(
            Param::integer("max_results")
                .min(1.0)
                .max(core::MAX_RESULTS as f64)
                .default(core::Options::default().max_results)
                .describe(MAX_RESULTS_DESC),
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
    name = "gizza-ai/scale-chord-finder",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Find the scales and modes that fit a set of notes, or spell any scale and its chords",
    skill(
        description = "Find the scales and modes that fit a set of notes, or spell any scale and its diatonic chords. With `notes` (letters A-G with optional # or b, separated by spaces or commas) the tool searches 42 scale types across all twelve roots and ranks the matches by how tightly they fit, reporting each scale's spelled notes, the notes it adds and the notes it misses. With `action=list`, `key` and `scale` it instead spells one scale: its notes, scale degrees, semitone and step patterns, its triads or sevenths with Roman numerals, and the other modes built from the same notes. `fit` chooses between scales that contain every note, match them exactly, or miss at most one. `root` narrows a search to one tonic, `spelling` forces sharps or flats, `chord_type` switches between triads, sevenths or both, `include_chords` and `include_modes` trim the report, and `max_results` caps how many matches are listed. `output` returns the full report, just the note names, CSV rows or JSON. Everything is computed from scale-degree specs, so notes and chords always agree on their spelling and the same options always give the same answer. Pure Rust/WASM; nothing is uploaded.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        // run_skill wraps the returned value in { "result": ... }. This is a
        // pure-text tool: every shape (report, names, csv, json) comes back as
        // a string through `output`, so no media envelope is involved.
        match run_skill(&body, "scale-chord-finder", |a: Args| {
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
        assert_eq!(props.len(), 12, "parameter count changed");
        for (name, spec) in props {
            let desc = spec["description"].as_str().unwrap_or("");
            assert!(desc.len() > 40, "param {name} needs a real description");
            assert!(
                spec.get("default").is_some(),
                "param {name} has no default, but nothing is required"
            );
        }
        // Nothing is required: a bare call still spells the default scale.
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
        assert_eq!(a.action, props["action"]["default"]);
        assert_eq!(a.notes, props["notes"]["default"]);
        assert_eq!(a.root, props["root"]["default"]);
        assert_eq!(a.key, props["key"]["default"]);
        assert_eq!(a.scale, props["scale"]["default"]);
        assert_eq!(a.fit, props["fit"]["default"]);
        assert_eq!(a.spelling, props["spelling"]["default"]);
        assert_eq!(a.include_chords, props["include_chords"]["default"]);
        assert_eq!(a.include_modes, props["include_modes"]["default"]);
        assert_eq!(a.chord_type, props["chord_type"]["default"]);
        assert_eq!(i64::from(a.max_results), props["max_results"]["default"]);
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
            out.starts_with("Scale: C major (Major (Ionian))\n"),
            "{out}"
        );
        assert!(out.contains("Notes:      C D  E   F  G A  B"), "{out}");
        assert!(out.contains("Triads:     C Dm Em  F  G Am Bdim"), "{out}");
    }

    /// Arguments really flow through to the core rather than being ignored, in
    /// both directions of the lookup.
    #[test]
    fn args_reach_the_core() {
        let a: Args = serde_json::from_str(
            r#"{"action":"list","key":"Eb","scale":"minor","output":"names"}"#,
        )
        .unwrap();
        assert_eq!(run_args(&a).unwrap(), "Eb F Gb Ab Bb Cb Db");

        // A bare `notes` flips `auto` over to the search direction.
        let a: Args = serde_json::from_str(r#"{"notes":"C E G B","max_results":5}"#).unwrap();
        let out = run_args(&a).unwrap();
        assert!(
            out.starts_with("Notes: C E G B (4 pitch classes)\n"),
            "{out}"
        );
        assert!(out.contains("E hirajoshi"), "{out}");

        let a: Args = serde_json::from_str(r#"{"notes":"C E G B","root":"G"}"#).unwrap();
        let out = run_args(&a).unwrap();
        assert!(out.contains("G "), "{out}");
        assert!(!out.contains("\nC major"), "root filter ignored: {out}");

        let a: Args = serde_json::from_str(r#"{"notes":"H"}"#).unwrap();
        assert!(run_args(&a).unwrap_err().contains("unknown note 'H'"));
        let a: Args = serde_json::from_str(r#"{"action":"list","key":"Q"}"#).unwrap();
        assert!(run_args(&a).unwrap_err().contains("key"));
    }

    /// Every enum variant the descriptor advertises must be one the core
    /// accepts — an advertised-but-rejected option is a broken dropdown. The
    /// two directions validate different params, so each variant is exercised
    /// through an action that actually reads it.
    #[test]
    fn every_advertised_enum_variant_is_accepted_by_the_core() {
        // `find`-side params (root/fit) need notes to search.
        for param in ["action", "root", "fit", "spelling", "chord_type", "output"] {
            for v in variants(param) {
                let json = format!(r#"{{"notes":"C E G B","{param}":"{v}"}}"#);
                let a: Args = serde_json::from_str(&json).unwrap();
                run_args(&a).unwrap_or_else(|e| panic!("find {param}={v}: {e}"));
            }
        }
        // `list`-side params (key/scale) need the listing direction.
        for param in ["key", "scale", "spelling", "chord_type", "output"] {
            for v in variants(param) {
                let json = format!(r#"{{"action":"list","{param}":"{v}"}}"#);
                let a: Args = serde_json::from_str(&json).unwrap();
                run_args(&a).unwrap_or_else(|e| panic!("list {param}={v}: {e}"));
            }
        }
    }

    /// The advertised numeric bounds must be exactly the ones the core enforces:
    /// a min/max the core rejects is a slider that fails at its own end stop.
    #[test]
    fn numeric_bounds_match_what_the_core_accepts() {
        let s = schema();
        let at = |v: f64| -> Result<String, String> {
            let a: Args =
                serde_json::from_str(&format!(r#"{{"notes":"C E G","max_results":{v}}}"#)).unwrap();
            run_args(&a)
        };
        let lo = s["properties"]["max_results"]["minimum"].as_f64().unwrap();
        let hi = s["properties"]["max_results"]["maximum"].as_f64().unwrap();
        at(lo).unwrap_or_else(|e| panic!("max_results={lo} rejected: {e}"));
        at(hi).unwrap_or_else(|e| panic!("max_results={hi} rejected: {e}"));
        assert!(
            at(lo - 1.0).is_err(),
            "max_results accepts below its minimum"
        );
        assert!(
            at(hi + 1.0).is_err(),
            "max_results accepts above its maximum"
        );
        assert_eq!(s["properties"]["max_results"]["maximum"], core::MAX_RESULTS);
    }

    /// The booleans are real switches: turning them off must change the report.
    #[test]
    fn boolean_switches_reach_the_core() {
        let full: Args =
            serde_json::from_str(r#"{"action":"list","key":"G","scale":"lydian"}"#).unwrap();
        let full = run_args(&full).unwrap();
        assert!(full.contains("Triads:"), "{full}");
        assert!(full.contains("Same notes as:"), "{full}");

        let bare: Args = serde_json::from_str(
            r#"{"action":"list","key":"G","scale":"lydian",
                "include_chords":false,"include_modes":false}"#,
        )
        .unwrap();
        let bare = run_args(&bare).unwrap();
        assert!(!bare.contains("Triads:"), "{bare}");
        assert!(!bare.contains("Same notes as:"), "{bare}");
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
                "action": {
                    "type": "string",
                    "enum": ["auto", "find", "list"],
                    "default": "auto",
                    "description": ACTION_DESC
                },
                "notes": {
                    "type": "string",
                    "default": "",
                    "description": NOTES_DESC
                },
                "root": {
                    "type": "string",
                    "enum": ["any", "C", "C#", "Db", "D", "D#", "Eb", "E", "F", "F#", "Gb",
                             "G", "G#", "Ab", "A", "A#", "Bb", "B"],
                    "default": "any",
                    "description": ROOT_DESC
                },
                "key": {
                    "type": "string",
                    "enum": ["C", "C#", "Db", "D", "D#", "Eb", "E", "F", "F#", "Gb",
                             "G", "G#", "Ab", "A", "A#", "Bb", "B"],
                    "default": "C",
                    "description": KEY_DESC
                },
                "scale": {
                    "type": "string",
                    "enum": ["major", "minor", "dorian", "phrygian", "lydian", "mixolydian",
                             "locrian", "major-pentatonic", "minor-pentatonic",
                             "egyptian-pentatonic", "blues", "major-blues", "harmonic-minor",
                             "locrian-natural6", "ionian-augmented", "ukrainian-dorian",
                             "phrygian-dominant", "lydian-sharp2", "altered-diminished",
                             "melodic-minor", "dorian-flat2", "lydian-augmented",
                             "lydian-dominant", "mixolydian-flat6", "locrian-natural2",
                             "altered", "harmonic-major", "double-harmonic", "hungarian-minor",
                             "neapolitan-minor", "neapolitan-major", "whole-tone", "augmented",
                             "diminished-whole-half", "diminished-half-whole", "bebop-dominant",
                             "bebop-major", "hirajoshi", "in-sen", "iwato", "kumoi", "chromatic"],
                    "default": "major",
                    "description": SCALE_DESC
                },
                "fit": {
                    "type": "string",
                    "enum": ["contains", "exact", "near"],
                    "default": "contains",
                    "description": FIT_DESC
                },
                "spelling": {
                    "type": "string",
                    "enum": ["auto", "sharps", "flats"],
                    "default": "auto",
                    "description": SPELLING_DESC
                },
                "include_chords": {
                    "type": "boolean", "default": true, "description": INCLUDE_CHORDS_DESC
                },
                "include_modes": {
                    "type": "boolean", "default": true, "description": INCLUDE_MODES_DESC
                },
                "chord_type": {
                    "type": "string",
                    "enum": ["triads", "sevenths", "both"],
                    "default": "triads",
                    "description": CHORD_TYPE_DESC
                },
                "max_results": {
                    "type": "integer", "minimum": 1, "maximum": 50,
                    "default": 12, "description": MAX_RESULTS_DESC
                },
                "output": {
                    "type": "string",
                    "enum": ["text", "names", "csv", "json"],
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
        assert_eq!(manifest["name"], "gizza-ai/scale-chord-finder");
        assert_eq!(manifest["summary"], MACRO_SUMMARY);
    }

    /// The summary shown in chat (the wafer_block macro), in manifest.json and
    /// in wafer.toml must stay identical — scripts/check-tool-hygiene.py fails
    /// the build when they diverge.
    const MACRO_SUMMARY: &str =
        "Find the scales and modes that fit a set of notes, or spell any scale and its chords";

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
