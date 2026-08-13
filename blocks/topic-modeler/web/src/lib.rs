//! Browser-facing wasm-bindgen wrapper for /tools/topic-modeler/.
//! Field order MUST match meta.toml: documents, separator, topics,
//! words_per_topic, iterations, alpha, beta, remove_stopwords, stopwords,
//! min_word_length, seed, output. Every field arrives as a raw string, so each
//! one is parsed here and clamped into its declared range — the page recomputes
//! on every keystroke, and a half-typed number shouldn't flash a red error.
use gizza_ai_topic_modeler_core::{run as model_run, Options};
use wasm_bindgen::prelude::*;

fn int_field(raw: &str, default: u32, min: u32, max: u32) -> u32 {
    raw.trim().parse::<u32>().unwrap_or(default).clamp(min, max)
}

fn float_field(raw: &str, default: f64, min: f64, max: f64) -> f64 {
    let v = raw.trim().parse::<f64>().unwrap_or(default);
    if v.is_finite() {
        v.clamp(min, max)
    } else {
        default
    }
}

#[wasm_bindgen]
#[allow(clippy::too_many_arguments)]
pub fn run(
    documents: &str,
    separator: &str,
    topics: &str,
    words_per_topic: &str,
    iterations: &str,
    alpha: &str,
    beta: &str,
    remove_stopwords: &str,
    stopwords: &str,
    min_word_length: &str,
    seed: &str,
    output: &str,
) -> Result<String, JsValue> {
    // Empty input → empty result (the page shows a neutral idle state rather
    // than a red error on first load / after Reset).
    if documents.trim().is_empty() {
        return Ok(String::new());
    }
    let pick = |raw: &str, default: &str| {
        let t = raw.trim();
        if t.is_empty() {
            default.to_string()
        } else {
            t.to_string()
        }
    };
    let opts = Options {
        separator: pick(separator, "blank-line"),
        topics: int_field(topics, 5, 2, 20),
        words_per_topic: int_field(words_per_topic, 8, 1, 25),
        iterations: int_field(iterations, 200, 50, 1000),
        alpha: float_field(alpha, 0.0, 0.0, 100.0),
        beta: float_field(beta, 0.01, 0.001, 1.0),
        // Page checkbox arrives as "true"/"false"; treat positive-truthy as on.
        remove_stopwords: matches!(remove_stopwords.trim(), "true" | "1" | "on" | "yes"),
        stopwords: stopwords.to_string(),
        min_word_length: int_field(min_word_length, 3, 1, 12),
        seed: seed.trim().parse::<u64>().unwrap_or(42),
        output: pick(output, "report"),
    };
    model_run(documents, &opts).map_err(|e| JsValue::from_str(&e))
}
