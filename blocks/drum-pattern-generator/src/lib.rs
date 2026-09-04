//! gizza-ai/drum-pattern-generator — chat skill block on the shared tool abstraction.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_drum_pattern_generator_core as core;
use serde::Deserialize;
use wafer_sdk::*;

const SUMMARY: &str =
    "Generate deterministic drum grooves as MIDI, WAV preview audio and an ASCII step grid";
const GENRES: [&str; 20] = [
    "rock",
    "pop",
    "funk",
    "disco",
    "house",
    "techno",
    "dnb",
    "breakbeat",
    "trap",
    "boom-bap",
    "lofi",
    "reggae",
    "reggaeton",
    "afrobeat",
    "bossa-nova",
    "jazz-swing",
    "blues-shuffle",
    "metal",
    "country",
    "waltz",
];
const TIME_SIGNATURES: [&str; 7] = ["4/4", "3/4", "2/4", "5/4", "6/8", "7/8", "12/8"];
const KITS: [&str; 8] = [
    "standard",
    "room",
    "power",
    "electronic",
    "tr808",
    "jazz",
    "brush",
    "orchestra",
];

#[derive(Deserialize, Debug)]
#[serde(default)]
struct Args {
    genre: String,
    time_signature: String,
    bars: u32,
    tempo: f64,
    complexity: String,
    hat_subdivision: String,
    swing: f64,
    humanize: f64,
    fill_every: u32,
    velocity: u8,
    kit: String,
    seed: u32,
    preview: String,
    output: String,
}

impl Default for Args {
    fn default() -> Self {
        let o = core::Options::default();
        Args {
            genre: o.genre,
            time_signature: o.time_signature,
            bars: o.bars,
            tempo: o.tempo,
            complexity: o.complexity,
            hat_subdivision: o.hat_subdivision,
            swing: o.swing,
            humanize: o.humanize,
            fill_every: o.fill_every,
            velocity: o.velocity,
            kit: o.kit,
            seed: o.seed,
            preview: o.preview,
            output: o.output,
        }
    }
}

impl From<&Args> for core::Options {
    fn from(a: &Args) -> Self {
        core::Options {
            genre: a.genre.clone(),
            time_signature: a.time_signature.clone(),
            bars: a.bars,
            tempo: a.tempo,
            complexity: a.complexity.clone(),
            hat_subdivision: a.hat_subdivision.clone(),
            swing: a.swing,
            humanize: a.humanize,
            fill_every: a.fill_every,
            velocity: a.velocity,
            kit: a.kit.clone(),
            seed: a.seed,
            preview: a.preview.clone(),
            output: a.output.clone(),
        }
    }
}

fn descriptor() -> ToolDescriptor {
    let d = core::Options::default();
    ToolDescriptor::new(Input::None)
        .param(Param::enumv("genre", GENRES).default(d.genre).describe("Genre preset for the groove. Each preset picks a typical tempo, its core voices and its feel — for example rock, funk, house, trap, dnb, boom-bap, reggaeton, bossa-nova, jazz-swing, blues-shuffle, metal, country or waltz."))
        .param(Param::enumv("time_signature", TIME_SIGNATURES).default(d.time_signature).describe("Meter for each bar. Supports common straight and compound signatures: 4/4, 3/4, 2/4, 5/4, 6/8, 7/8 and 12/8."))
        .param(Param::integer("bars").min(1.0).max(core::MAX_BARS as f64).default(d.bars).describe("Number of bars to generate, from 1 to 64. MIDI always covers every bar; long WAV previews are truncated to a safe length."))
        .param(Param::number("tempo").min(0.0).max(300.0).default(d.tempo).describe("Tempo in BPM. Use 0 to take the genre's typical tempo, or choose an explicit tempo from 20 to 300 BPM."))
        .param(Param::enumv("complexity", core::COMPLEXITIES).default(d.complexity).describe("Pattern density: basic drops embellishments, standard gives a usable groove, and busy adds extra kicks, ghosts or percussion where the genre supports it."))
        .param(Param::enumv("hat_subdivision", core::SUBDIVISIONS).default(d.hat_subdivision).describe("Grid used for hats and the ASCII preview. Auto follows the genre/complexity, or force quarter, eighth, sixteenth or triplet-eighth."))
        .param(Param::number("swing").min(0.0).max(75.0).default(d.swing).describe("Swing amount in percent, 0 to 75. Off-beat grid hits are delayed deterministically; triplet-eighth grooves keep their natural shuffle."))
        .param(Param::number("humanize").min(0.0).max(100.0).default(d.humanize).describe("Deterministic timing and velocity variation, 0 to 100 percent. The seed controls the pseudo-random offsets so repeated runs are byte-identical."))
        .param(Param::integer("fill_every").min(0.0).max(core::MAX_BARS as f64).default(d.fill_every).describe("Add a tom/snare fill on every Nth bar. Use 0 for no fills; values above the bar count simply produce no fill."))
        .param(Param::integer("velocity").min(1.0).max(127.0).default(d.velocity).describe("Base MIDI velocity from 1 to 127. Accents and ghost notes are scaled around this value and clamped to the MIDI range."))
        .param(Param::enumv("kit", KITS).default(d.kit).describe("General MIDI drum kit program: standard, room, power, electronic, tr808, jazz, brush or orchestra. The kit selects the GM program number; notes stay on channel 10 percussion."))
        .param(Param::integer("seed").min(0.0).default(d.seed).describe("Reproducible variation seed for humanize and audio noise. The same seed and parameters produce byte-identical MIDI and WAV output."))
        .param(Param::enumv("preview", core::PREVIEWS).default(d.preview).describe("Rendered audio preview mode: drums only, drums plus metronome click, click only, or off. WAV previews are capped near 30 seconds."))
        .param(Param::enumv("output", core::OUTPUTS).default(d.output).describe("Returned shape: full report with grid, grid only, MIDI base64, WAV base64, or JSON containing summary fields and both base64 artifacts."))
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}
fn run_args(args: &Args) -> Result<String, String> {
    core::run(&core::Options::from(args))
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/drum-pattern-generator",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Generate deterministic drum grooves as MIDI, WAV preview audio and an ASCII step grid",
    skill(
        description = "Generate a deterministic drum pattern from genre, time signature, bar count, tempo, complexity, subdivision, swing, humanize, fill, velocity, kit and seed controls. The tool emits General MIDI drum data on channel 10, a pure-Rust rendered WAV preview when enabled, and an ASCII step grid for text surfaces. The same parameters always produce byte-identical MIDI and WAV output.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "drum-pattern-generator", |a: Args| {
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

    #[test]
    fn descriptor_has_real_params_and_defaults() {
        let s = schema();
        let props = s["properties"].as_object().unwrap();
        assert_eq!(props.len(), 14);
        assert!(s.get("required").is_none());
        for (name, spec) in props {
            assert!(
                spec["description"].as_str().unwrap_or("").len() > 40,
                "{name}"
            );
            assert!(spec.get("default").is_some(), "{name}");
        }
        assert_eq!(
            props["genre"]["enum"].as_array().unwrap().len(),
            GENRES.len()
        );
        assert_eq!(props["preview"]["enum"], serde_json::json!(core::PREVIEWS));
        assert_eq!(props["output"]["enum"], serde_json::json!(core::OUTPUTS));
    }

    /// The descriptor's enum lists are the page/CLI/chat vocabulary; the core
    /// tables are what actually validates. A value advertised in one and
    /// missing from the other is an unusable choice, so pin them together.
    #[test]
    fn advertised_enum_values_all_exist_in_core() {
        assert_eq!(GENRES.to_vec(), core::genre_keys());
        assert_eq!(KITS.to_vec(), core::kit_keys());
        assert_eq!(TIME_SIGNATURES.to_vec(), core::time_signature_keys());
        // Every advertised value must survive a real run.
        for genre in GENRES {
            let a: Args = serde_json::from_str(&format!(
                r#"{{"genre":"{genre}","bars":1,"preview":"off"}}"#
            ))
            .unwrap();
            run_args(&a).unwrap_or_else(|e| panic!("genre {genre}: {e}"));
        }
        for kit in KITS {
            let a: Args =
                serde_json::from_str(&format!(r#"{{"kit":"{kit}","bars":1,"preview":"off"}}"#))
                    .unwrap();
            run_args(&a).unwrap_or_else(|e| panic!("kit {kit}: {e}"));
        }
    }

    #[test]
    fn defaults_and_args_reach_core() {
        let a: Args = serde_json::from_str("{}").unwrap();
        let out = run_args(&a).unwrap();
        assert!(out.starts_with("Rock pattern in 4/4"), "{out}");
        assert!(out.contains("Kick"));
        let a: Args =
            serde_json::from_str(r#"{"genre":"trap","bars":1,"preview":"off","output":"grid"}"#)
                .unwrap();
        let out = run_args(&a).unwrap();
        assert!(out.contains("Kick"));
        assert!(!out.contains("pattern in"));
        let a: Args = serde_json::from_str(r#"{"genre":"polka"}"#).unwrap();
        assert!(run_args(&a).unwrap_err().contains("unknown genre 'polka'"));
    }

    #[test]
    fn manifest_tool_section_matches_descriptor() {
        let manifest: serde_json::Value =
            serde_json::from_str(include_str!("../manifest.json")).unwrap();
        assert_eq!(manifest["summary"], SUMMARY);
        assert_eq!(manifest["tool"]["parameters"], schema());
    }

    #[test]
    fn summaries_agree() {
        assert!(include_str!("lib.rs").contains(&format!("summary = \"{SUMMARY}\"")));
        assert!(include_str!("../wafer.toml").contains(&format!("summary = \"{SUMMARY}\"")));
    }
}
