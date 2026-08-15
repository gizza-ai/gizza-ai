//! Browser-facing wasm-bindgen wrapper for /tools/midi-track-splitter/.
//!
//! Every argument arrives from the form as a string (the page driver passes
//! field values as strings), so the checkboxes are read positive-truthy here.
//! The tool's real output is a SET of binary MIDI files, so `run` returns the
//! same JSON document the chat/CLI surface returns — a summary, the parts
//! table, and one `data:audio/midi;base64,…` URL per part — and
//! `page/custom.js` turns that into a readable table with a Download button
//! per part.
//!
//! Argument order MUST match the `[[input]]` order in `page/meta.toml`.

use gizza_ai_midi_track_splitter_core::{split_to_json, truthy, Options};
use wasm_bindgen::prelude::*;

/// An empty select falls back to the schema default.
fn or_default<'a>(v: &'a str, default: &'a str) -> &'a str {
    if v.trim().is_empty() {
        default
    } else {
        v.trim()
    }
}

#[wasm_bindgen]
#[allow(clippy::too_many_arguments)]
pub fn run(
    input: &str,
    encoding: &str,
    split_by: &str,
    include_conductor: &str,
    output_format: &str,
    skip_empty: &str,
    select: &str,
    filename_prefix: &str,
    output: &str,
) -> Result<String, JsValue> {
    // Empty input → empty result, so the page shows a neutral idle prompt
    // rather than a red error on first load / after Reset.
    if input.trim().is_empty() {
        return Ok(String::new());
    }
    let d = Options::default();
    let opts = Options::parse(
        or_default(split_by, "track"),
        truthy(include_conductor, d.include_conductor),
        or_default(output_format, "format-0"),
        truthy(skip_empty, d.skip_empty),
        select,
        or_default(filename_prefix, "part"),
        or_default(output, "files"),
    )
    .map_err(|e| JsValue::from_str(&e))?;

    split_to_json(input, or_default(encoding, "auto"), &opts).map_err(|e| JsValue::from_str(&e))
}
