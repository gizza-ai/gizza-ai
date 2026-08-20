//! Browser-facing wasm-bindgen wrapper for /tools/accent-stripper/.
use gizza_ai_accent_stripper_core::{strip, Mode, Options, Unmapped};
use wasm_bindgen::prelude::*;

fn truthy(s: &str) -> bool {
    matches!(
        s.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "on" | "yes"
    )
}

fn mode_of(s: &str) -> Result<Mode, JsValue> {
    Mode::parse(s).map_err(|e| JsValue::from_str(&e))
}

fn unmapped_of(s: &str) -> Result<Unmapped, JsValue> {
    Unmapped::parse(s).map_err(|e| JsValue::from_str(&e))
}

fn json_string(s: &str) -> String {
    serde_json::to_string(s).expect("string serialization cannot fail")
}

#[allow(clippy::too_many_arguments)]
#[wasm_bindgen]
pub fn run(
    input: &str,
    mode: &str,
    unmapped: &str,
    replacement: &str,
    keep: &str,
    lowercase: &str,
    collapse_whitespace: &str,
    include_report: &str,
) -> Result<String, JsValue> {
    let opts = Options {
        mode: mode_of(mode)?,
        unmapped: unmapped_of(unmapped)?,
        replacement: if replacement.trim().is_empty() {
            "?".to_string()
        } else {
            replacement.to_string()
        },
        keep: keep.to_string(),
        lowercase: truthy(lowercase),
        collapse_whitespace: truthy(collapse_whitespace),
    };
    let report = strip(input, &opts).map_err(|e| JsValue::from_str(&e))?;
    if truthy(include_report) {
        Ok(format!(
            "{{\n  \"result\": {},\n  \"input_chars\": {},\n  \"output_chars\": {},\n  \"non_ascii_in\": {},\n  \"converted\": {},\n  \"kept\": {},\n  \"unmapped\": {},\n  \"output_is_ascii\": {},\n  \"unmapped_samples\": [{}]\n}}",
            json_string(&report.result),
            report.input_chars,
            report.output_chars,
            report.non_ascii_in,
            report.converted,
            report.kept,
            report.unmapped,
            report.output_is_ascii,
            report.unmapped_samples.iter().map(|s| json_string(s)).collect::<Vec<_>>().join(", ")
        ))
    } else {
        Ok(report.result)
    }
}
