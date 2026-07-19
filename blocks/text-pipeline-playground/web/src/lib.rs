//! Browser-facing wasm-bindgen wrapper for /tools/text-pipeline-playground/.
//! Field order MUST match meta.toml: text, pipeline, regex_mode,
//! case_insensitive, limit, on_error. All fields arrive as strings (checkboxes
//! send "true"/"false"; number fields arrive as text).
use gizza_ai_text_pipeline_playground_core::{run as run_pipeline, OnError, Options};
use wasm_bindgen::prelude::*;

fn truthy(s: &str) -> bool {
    matches!(s.trim().to_ascii_lowercase().as_str(), "true" | "1" | "on" | "yes")
}

#[wasm_bindgen]
pub fn run(
    text: &str,
    pipeline: &str,
    regex_mode: &str,
    case_insensitive: &str,
    limit: &str,
    on_error: &str,
) -> Result<String, JsValue> {
    let limit_txt = limit.trim();
    let limit: usize = if limit_txt.is_empty() {
        10_000
    } else {
        limit_txt
            .parse::<usize>()
            .map_err(|_| JsValue::from_str(&format!("limit must be a whole number (got {limit_txt:?})")))?
    };
    if limit < 1 || limit > 1_000_000 {
        return Err(JsValue::from_str("limit must be between 1 and 1000000"));
    }
    let opts = Options {
        regex_mode: truthy(regex_mode),
        case_insensitive: truthy(case_insensitive),
        limit,
        on_error: OnError::parse(on_error).map_err(|e| JsValue::from_str(&e))?,
    };
    run_pipeline(text, pipeline, &opts).map_err(|e| JsValue::from_str(&e))
}
