//! Browser-facing wasm-bindgen wrapper for /tools/text-normalizer/.
//! Field order MUST match meta.toml: text, form, case, strip_accents,
//! whitespace, punctuation. Fields arrive as strings (checkboxes send
//! "true"/"false"; an empty select falls back to the core parse default).
use gizza_ai_text_normalizer_core::{normalize, Case, Form, Options, Whitespace};
use wasm_bindgen::prelude::*;

fn truthy(s: &str) -> bool {
    matches!(s.trim().to_ascii_lowercase().as_str(), "true" | "1" | "on" | "yes")
}

#[wasm_bindgen]
pub fn run(
    text: &str,
    form: &str,
    case: &str,
    strip_accents: &str,
    whitespace: &str,
    punctuation: &str,
) -> Result<String, JsValue> {
    let opts = Options {
        form: Form::parse(form).map_err(|e| JsValue::from_str(&e))?,
        case: Case::parse(case).map_err(|e| JsValue::from_str(&e))?,
        strip_accents: truthy(strip_accents),
        whitespace: Whitespace::parse(whitespace).map_err(|e| JsValue::from_str(&e))?,
        punctuation: truthy(punctuation),
    };
    Ok(normalize(text, opts))
}
