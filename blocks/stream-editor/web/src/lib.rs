//! Browser-facing wasm-bindgen wrapper for /tools/stream-editor/.
use gizza_ai_stream_editor_core::{LineEnding, Options, RegexFlavor, DEFAULT_MAX_OUTPUT_LINES};
use wasm_bindgen::prelude::*;

fn boolish(s: &str) -> bool {
    matches!(
        s.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "on" | "yes"
    )
}

fn whole(v: f64, name: &str) -> Result<usize, JsValue> {
    if !v.is_finite() {
        return Ok(DEFAULT_MAX_OUTPUT_LINES);
    }
    if v < 1.0 || v.fract() != 0.0 {
        return Err(JsValue::from_str(&format!(
            "{name} must be a whole number at least 1"
        )));
    }
    Ok(v as usize)
}

#[wasm_bindgen]
pub fn run(
    text: &str,
    script: &str,
    quiet: &str,
    ignore_case: &str,
    whole_buffer: &str,
    regex_flavor: &str,
    line_ending: &str,
    max_output_lines: f64,
) -> Result<String, JsValue> {
    let opts = Options {
        quiet: boolish(quiet),
        ignore_case: boolish(ignore_case),
        whole_buffer: boolish(whole_buffer),
        flavor: RegexFlavor::parse(regex_flavor).map_err(|e| JsValue::from_str(&e))?,
        line_ending: LineEnding::parse(line_ending).map_err(|e| JsValue::from_str(&e))?,
        max_output_lines: whole(max_output_lines, "max_output_lines")?,
    };
    gizza_ai_stream_editor_core::run(text, script, &opts).map_err(|e| JsValue::from_str(&e))
}
