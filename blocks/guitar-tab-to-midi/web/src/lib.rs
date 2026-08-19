//! Browser-facing wasm-bindgen wrapper for /tools/guitar-tab-to-midi/.
//!
//! Every argument arrives from the form as a string (the page driver passes
//! field values as strings), so numbers are parsed here and the checkbox is
//! read positive-truthy. The tool's real output is BINARY MIDI, so `run`
//! returns a small JSON envelope — a human summary plus a `data:` URL — and
//! `page/custom.js` turns that into a Download button.
//!
//! Argument order MUST match the `[[input]]` order in `page/meta.toml`.

use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
use gizza_ai_guitar_tab_to_midi_core::{convert, parse_field, truthy, Options};
use wasm_bindgen::prelude::*;

/// Standard MIDI File MIME type.
const MIDI_MIME: &str = "audio/midi";
/// Download name for the produced file.
const MIDI_FILENAME: &str = "guitar-tab.mid";

/// An empty select falls back to the schema default.
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
    tab: &str,
    tuning: &str,
    custom_tuning: &str,
    string_order: &str,
    capo: &str,
    transpose: &str,
    tempo: &str,
    note_duration: &str,
    timing: &str,
    sustain: &str,
    velocity: &str,
    instrument: &str,
    muted_notes: &str,
) -> Result<String, JsValue> {
    // Empty input → empty result, so the page shows a neutral idle prompt
    // rather than a red error on first load / after Reset.
    if tab.trim().is_empty() {
        return Ok(String::new());
    }
    let d = Options::default();
    let opts = Options {
        tuning: or_default(tuning, &d.tuning),
        custom_tuning: custom_tuning.to_string(),
        string_order: or_default(string_order, &d.string_order),
        capo: parse_field("capo", capo, d.capo).map_err(|e| JsValue::from_str(&e))?,
        transpose: parse_field("transpose", transpose, d.transpose)
            .map_err(|e| JsValue::from_str(&e))?,
        tempo: parse_field("tempo", tempo, d.tempo).map_err(|e| JsValue::from_str(&e))?,
        note_duration: or_default(note_duration, &d.note_duration),
        timing: or_default(timing, &d.timing),
        sustain: or_default(sustain, &d.sustain),
        velocity: parse_field("velocity", velocity, d.velocity)
            .map_err(|e| JsValue::from_str(&e))?,
        instrument: or_default(instrument, &d.instrument),
        muted_notes: truthy(muted_notes, d.muted_notes),
    };

    let out = convert(tab, &opts).map_err(|e| JsValue::from_str(&e))?;
    let payload = serde_json::json!({
        "summary": out.summary(),
        "notes": out.notes,
        "staves": out.staves,
        "strings": out.strings,
        "seconds": out.seconds,
        "bytes": out.midi.len(),
        "tuning": out.tuning_name,
        "tuning_notes": out.tuning_names(),
        "lowest": gizza_ai_guitar_tab_to_midi_core::midi_to_name(out.lowest as i32),
        "highest": gizza_ai_guitar_tab_to_midi_core::midi_to_name(out.highest as i32),
        "filename": MIDI_FILENAME,
        "data_url": format!("data:{MIDI_MIME};base64,{}", B64.encode(&out.midi)),
    });
    Ok(payload.to_string())
}
