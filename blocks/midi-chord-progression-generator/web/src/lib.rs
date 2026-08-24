//! Browser-facing wasm-bindgen wrapper for /tools/midi-chord-progression-generator/.
//!
//! The tool's real output is binary MIDI, so `run` returns a small JSON envelope
//! with a human summary plus a data:audio/midi URL. `page/custom.js` turns it
//! into a readable summary and a Download button.
use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
use gizza_ai_midi_chord_progression_generator_core::{convert, parse_field, truthy, Options};
use wasm_bindgen::prelude::*;

const MIDI_MIME: &str = "audio/midi";
const MIDI_FILENAME: &str = "chord-progression.mid";

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
    progression: &str,
    tempo: &str,
    beats_per_chord: &str,
    beats_per_bar: &str,
    octave: &str,
    voicing: &str,
    inversion: &str,
    pattern: &str,
    arp_note: &str,
    note_length: &str,
    add_bass: &str,
    transpose: &str,
    velocity: &str,
    instrument: &str,
) -> Result<String, JsValue> {
    if progression.trim().is_empty() {
        return Ok(String::new());
    }
    let d = Options::default();
    let opts = Options {
        tempo: parse_field("tempo", tempo, d.tempo).map_err(|e| JsValue::from_str(&e))?,
        beats_per_chord: parse_field("beats_per_chord", beats_per_chord, d.beats_per_chord)
            .map_err(|e| JsValue::from_str(&e))?,
        beats_per_bar: parse_field("beats_per_bar", beats_per_bar, d.beats_per_bar)
            .map_err(|e| JsValue::from_str(&e))?,
        octave: parse_field("octave", octave, d.octave).map_err(|e| JsValue::from_str(&e))?,
        voicing: or_default(voicing, &d.voicing),
        inversion: or_default(inversion, &d.inversion),
        pattern: or_default(pattern, &d.pattern),
        arp_note: or_default(arp_note, &d.arp_note),
        note_length: parse_field("note_length", note_length, d.note_length)
            .map_err(|e| JsValue::from_str(&e))?,
        add_bass: truthy(add_bass, d.add_bass),
        transpose: parse_field("transpose", transpose, d.transpose)
            .map_err(|e| JsValue::from_str(&e))?,
        velocity: parse_field("velocity", velocity, d.velocity)
            .map_err(|e| JsValue::from_str(&e))?,
        instrument: or_default(instrument, &d.instrument),
    };

    let out = convert(progression, &opts).map_err(|e| JsValue::from_str(&e))?;
    let payload = serde_json::json!({
        "summary": out.summary(),
        "detail": out.detail_text(),
        "notes": out.notes,
        "slots": out.slots,
        "chords": out.chords,
        "beats": out.beats,
        "seconds": out.seconds,
        "bytes": out.midi.len(),
        "lowest": gizza_ai_midi_chord_progression_generator_core::midi_to_name(out.lowest as i32),
        "highest": gizza_ai_midi_chord_progression_generator_core::midi_to_name(out.highest as i32),
        "filename": MIDI_FILENAME,
        "data_url": format!("data:{MIDI_MIME};base64,{}", B64.encode(&out.midi)),
    });
    Ok(payload.to_string())
}
