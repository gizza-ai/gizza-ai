//! Browser-facing wasm-bindgen wrapper for /tools/full-text-search/.
//! Field order MUST match page/meta.toml: corpus, query, separator, algorithm,
//! stemming, stopwords, match_mode, prefix, max_results, snippet_words, k1, b,
//! title_boost, output. Checkboxes arrive as strings.
use gizza_ai_full_text_search_core::{
    search_text, Algorithm, MatchMode, Options, OutputFormat, Separator,
};
use wasm_bindgen::prelude::*;

fn truthy(s: &str, default: bool) -> bool {
    let t = s.trim();
    if t.is_empty() {
        return default;
    }
    matches!(t.to_ascii_lowercase().as_str(), "true" | "1" | "on" | "yes")
}

fn int_value(s: &str, default: usize) -> usize {
    s.trim().parse::<usize>().unwrap_or(default)
}

fn float_value(s: &str, default: f64) -> f64 {
    s.trim().parse::<f64>().unwrap_or(default)
}

#[wasm_bindgen]
pub fn run(
    corpus: &str,
    query: &str,
    separator: &str,
    algorithm: &str,
    stemming: &str,
    stopwords: &str,
    match_mode: &str,
    prefix: &str,
    max_results: &str,
    snippet_words: &str,
    k1: &str,
    b: &str,
    title_boost: &str,
    output: &str,
) -> Result<String, JsValue> {
    let opts = Options {
        separator: Separator::parse(separator).map_err(|e| JsValue::from_str(&e))?,
        algorithm: Algorithm::parse(algorithm).map_err(|e| JsValue::from_str(&e))?,
        stemming: truthy(stemming, true),
        stopwords: truthy(stopwords, true),
        match_mode: MatchMode::parse(match_mode).map_err(|e| JsValue::from_str(&e))?,
        prefix: truthy(prefix, false),
        max_results: int_value(max_results, 10),
        snippet_words: int_value(snippet_words, 30),
        k1: float_value(k1, 1.2),
        b: float_value(b, 0.75),
        title_boost: float_value(title_boost, 2.0),
        output: OutputFormat::parse(output).map_err(|e| JsValue::from_str(&e))?,
    };
    search_text(query, corpus, opts).map_err(|e| JsValue::from_str(&e))
}
