//! Browser-facing wasm-bindgen wrapper for /tools/chord-progression-generator/.
//!
//! The page passes every control through as a string in the order they are
//! declared in `page/meta.toml`, so this mirrors the block descriptor's params
//! one-for-one and hands the parsed values to the same core entry point the
//! chat and CLI surfaces use. Blank fields fall back to the core's defaults.
use gizza_ai_chord_progression_generator_core as core;
use wasm_bindgen::prelude::*;

/// A blank select/text field means "keep the default" rather than "empty".
fn or_default(v: &str, default: &str) -> String {
    if v.trim().is_empty() {
        default.to_string()
    } else {
        v.trim().to_string()
    }
}

#[wasm_bindgen]
#[allow(clippy::too_many_arguments)]
pub fn run(
    key: &str,
    mode: &str,
    style: &str,
    variation: &str,
    sevenths: &str,
    borrowed: &str,
    chords: &str,
    tempo: &str,
    instrument: &str,
    pattern: &str,
    voice_leading: &str,
    repeats: &str,
    octave: &str,
    output: &str,
) -> Result<String, JsValue> {
    let d = core::Options::default();
    let opts = core::Options {
        key: or_default(key, &d.key),
        mode: or_default(mode, &d.mode),
        style: or_default(style, &d.style),
        variation: core::parse_field("variation", variation, d.variation)
            .map_err(|e| JsValue::from_str(&e))?,
        sevenths: or_default(sevenths, &d.sevenths),
        borrowed: or_default(borrowed, &d.borrowed),
        chords: core::parse_field("chords", chords, d.chords).map_err(|e| JsValue::from_str(&e))?,
        tempo: core::parse_field("tempo", tempo, d.tempo).map_err(|e| JsValue::from_str(&e))?,
        instrument: or_default(instrument, &d.instrument),
        pattern: or_default(pattern, &d.pattern),
        voice_leading: core::truthy(voice_leading, d.voice_leading),
        repeats: core::parse_field("repeats", repeats, d.repeats)
            .map_err(|e| JsValue::from_str(&e))?,
        octave: core::parse_field("octave", octave, d.octave).map_err(|e| JsValue::from_str(&e))?,
        output: or_default(output, &d.output),
    };
    core::run(&opts).map_err(|e| JsValue::from_str(&e))
}
