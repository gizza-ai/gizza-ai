//! Browser-facing wasm-bindgen wrapper for /tools/chat-log-analyzer/.
//! The page passes every field as a string (in declared meta.toml order); this
//! parses the numeric/boolean options and delegates to the pure core.
use gizza_ai_chat_log_analyzer_core::{analyze, OutputFormat};
use wasm_bindgen::prelude::*;

fn truthy(s: &str) -> bool {
    matches!(
        s.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "on" | "yes"
    )
}

#[wasm_bindgen]
pub fn run(
    log: &str,
    output: &str,
    top: &str,
    min_word_length: &str,
    ignore_stopwords: &str,
    exclude_nicks: &str,
) -> Result<String, JsValue> {
    let output = if output.trim().is_empty() {
        "summary"
    } else {
        output
    };
    let top = top.trim().parse::<usize>().unwrap_or(10);
    let min_word_length = min_word_length.trim().parse::<usize>().unwrap_or(3);
    // Default true (the checkbox loads checked); only an explicit off unticks it.
    let ignore = if ignore_stopwords.trim().is_empty() {
        true
    } else {
        truthy(ignore_stopwords)
    };
    analyze(
        log,
        OutputFormat::parse(output),
        top,
        min_word_length,
        ignore,
        exclude_nicks,
    )
    .map_err(|e| JsValue::from_str(&e))
}
