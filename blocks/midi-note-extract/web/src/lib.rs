//! Browser-facing wasm-bindgen wrapper for /tools/midi-note-extract/.
//! Field order MUST match meta.toml: input, encoding, columns, time_unit,
//! velocity_scale, delimiter, header, sort, track, channel, decimals.
//! Fields arrive as strings (checkboxes send "true"/"false").
use gizza_ai_midi_note_extract_core::{extract, Options};
use wasm_bindgen::prelude::*;

fn truthy(s: &str) -> bool {
    matches!(
        s.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "on" | "yes"
    )
}

fn parse_decimals(s: &str) -> Result<i64, JsValue> {
    let t = s.trim();
    if t.is_empty() {
        return Ok(3);
    }
    t.parse::<i64>()
        .map_err(|_| JsValue::from_str("decimals must be a whole number between 0 and 6"))
}

#[wasm_bindgen]
#[allow(clippy::too_many_arguments)]
pub fn run(
    input: &str,
    encoding: &str,
    columns: &str,
    time_unit: &str,
    velocity_scale: &str,
    delimiter: &str,
    header: &str,
    sort: &str,
    track: &str,
    channel: &str,
    decimals: &str,
) -> Result<String, JsValue> {
    let opts = Options::parse(
        columns,
        time_unit,
        velocity_scale,
        delimiter,
        truthy(header),
        track,
        channel,
        parse_decimals(decimals)?,
        sort,
    )
    .map_err(|e| JsValue::from_str(&e))?;
    extract(input, encoding, &opts).map_err(|e| JsValue::from_str(&e))
}
