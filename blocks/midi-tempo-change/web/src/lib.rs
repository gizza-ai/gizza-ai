//! Browser-facing wasm-bindgen wrapper for /tools/midi-tempo-change/.
//!
//! Every argument arrives from the form as a string (the page driver passes
//! field values as strings), so numbers are parsed here and the checkbox is
//! read positive-truthy. The tool's real output is BINARY MIDI, so `run`
//! returns a small JSON envelope — the numbers describing the change plus a
//! `data:` URL — and `page/custom.js` turns that into a Download button.
//!
//! Argument order MUST match the `[[input]]` order in `page/meta.toml`.

use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
use gizza_ai_midi_tempo_change_core::{change_tempo, parse_field, truthy, Options};
use wasm_bindgen::prelude::*;

/// Standard MIDI File MIME type.
const MIDI_MIME: &str = "audio/midi";
/// Download name for the retimed file.
const MIDI_FILENAME: &str = "tempo-changed.mid";

/// An empty select falls back to the schema default.
fn or_default<'a>(v: &'a str, default: &'a str) -> &'a str {
    if v.trim().is_empty() {
        default
    } else {
        v.trim()
    }
}

#[wasm_bindgen]
pub fn run(
    input: &str,
    encoding: &str,
    mode: &str,
    bpm: &str,
    factor: &str,
    tempo_map: &str,
    keep_duration: &str,
) -> Result<String, JsValue> {
    // Empty input → empty result, so the page shows a neutral idle prompt
    // rather than a red error on first load / after Reset.
    if input.trim().is_empty() {
        return Ok(String::new());
    }
    let d = Options::default();
    let opts = Options::parse(
        or_default(mode, "set-bpm"),
        parse_field("bpm", bpm, d.bpm).map_err(|e| JsValue::from_str(&e))?,
        parse_field("factor", factor, d.factor).map_err(|e| JsValue::from_str(&e))?,
        or_default(tempo_map, "scale"),
        truthy(keep_duration, d.keep_duration),
    )
    .map_err(|e| JsValue::from_str(&e))?;

    let out = change_tempo(input, or_default(encoding, "auto"), &opts)
        .map_err(|e| JsValue::from_str(&e))?;
    let payload = serde_json::json!({
        "summary": out.summary(),
        "original_bpm": out.original_bpm,
        "new_bpm": out.new_bpm,
        "ratio": out.ratio,
        "tempo_events_in": out.tempo_events_in,
        "tempo_events_out": out.tempo_events_out,
        "tracks": out.tracks,
        "ppq": out.ppq,
        "notes": out.notes,
        "seconds_before": out.seconds_before,
        "seconds_after": out.seconds_after,
        "keep_duration": out.keep_duration,
        "flattened": out.flattened,
        "bytes": out.midi.len(),
        "filename": MIDI_FILENAME,
        "data_url": format!("data:{MIDI_MIME};base64,{}", B64.encode(&out.midi)),
    });
    Ok(payload.to_string())
}
