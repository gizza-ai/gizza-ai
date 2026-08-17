//! Browser-facing wasm-bindgen wrapper for /tools/markdown-runbook-extractor/.
//! Field order MUST match meta.toml: markdown, language, output, tags,
//! strip_prompts, echo_steps, fail_fast, skip_marked. The page driver passes
//! EVERY field as a string, so booleans are parsed here.
use gizza_ai_markdown_runbook_extractor_core::{extract, Language, Options, OutputFormat};
use wasm_bindgen::prelude::*;

/// The page sends "true"/"false"; be lenient about the other truthy spellings.
fn truthy(s: &str, default: bool) -> bool {
    match s.trim().to_ascii_lowercase().as_str() {
        "" => default,
        "true" | "1" | "on" | "yes" => true,
        _ => false,
    }
}

#[wasm_bindgen]
pub fn run(
    markdown: &str,
    language: &str,
    output: &str,
    tags: &str,
    strip_prompts: &str,
    echo_steps: &str,
    fail_fast: &str,
    skip_marked: &str,
) -> Result<String, JsValue> {
    let opts = Options {
        language: Language::parse(language).map_err(|e| JsValue::from_str(&e))?,
        output: OutputFormat::parse(output).map_err(|e| JsValue::from_str(&e))?,
        tags: tags.to_string(),
        strip_prompts: truthy(strip_prompts, true),
        echo_steps: truthy(echo_steps, true),
        fail_fast: truthy(fail_fast, true),
        skip_marked: truthy(skip_marked, true),
    };
    extract(markdown, &opts).map_err(|e| JsValue::from_str(&e))
}
