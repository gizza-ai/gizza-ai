//! Browser-facing wasm-bindgen wrapper for /tools/drum-pattern-generator/.
use gizza_ai_drum_pattern_generator_core as core;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
#[allow(clippy::too_many_arguments)]
pub fn run(
    genre: &str,
    time_signature: &str,
    bars: &str,
    tempo: &str,
    complexity: &str,
    hat_subdivision: &str,
    swing: &str,
    humanize: &str,
    fill_every: &str,
    velocity: &str,
    kit: &str,
    seed: &str,
    preview: &str,
    output: &str,
) -> Result<String, JsValue> {
    let d = core::Options::default();
    let opts = core::Options {
        genre: core::or_default(genre, &d.genre),
        time_signature: core::or_default(time_signature, &d.time_signature),
        bars: core::parse_field("bars", bars, d.bars).map_err(|e| JsValue::from_str(&e))?,
        tempo: core::parse_field("tempo", tempo, d.tempo).map_err(|e| JsValue::from_str(&e))?,
        complexity: core::or_default(complexity, &d.complexity),
        hat_subdivision: core::or_default(hat_subdivision, &d.hat_subdivision),
        swing: core::parse_field("swing", swing, d.swing).map_err(|e| JsValue::from_str(&e))?,
        humanize: core::parse_field("humanize", humanize, d.humanize)
            .map_err(|e| JsValue::from_str(&e))?,
        fill_every: core::parse_field("fill_every", fill_every, d.fill_every)
            .map_err(|e| JsValue::from_str(&e))?,
        velocity: core::parse_field("velocity", velocity, d.velocity)
            .map_err(|e| JsValue::from_str(&e))?,
        kit: core::or_default(kit, &d.kit),
        seed: core::parse_field("seed", seed, d.seed).map_err(|e| JsValue::from_str(&e))?,
        preview: core::or_default(preview, &d.preview),
        output: core::or_default(output, &d.output),
    };
    core::run(&opts).map_err(|e| JsValue::from_str(&e))
}
