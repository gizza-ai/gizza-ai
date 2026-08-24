//! Browser-facing wasm-bindgen wrapper for /tools/few-shot-text-classifier/.
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
#[allow(clippy::too_many_arguments)]
pub fn run(
    examples: &str,
    text: &str,
    separator: &str,
    input_mode: &str,
    method: &str,
    k: &str,
    similarity: &str,
    weighting: &str,
    analyzer: &str,
    ngram_max: &str,
    lowercase: &str,
    strip_accents: &str,
    remove_stopwords: &str,
    sublinear_tf: &str,
    min_df: &str,
    min_confidence: &str,
    top_k: &str,
    explain: &str,
    output: &str,
) -> Result<String, JsValue> {
    let opts = gizza_ai_few_shot_text_classifier_core::Options {
        separator: default_str(separator, "auto").to_string(),
        input_mode: default_str(input_mode, "single").to_string(),
        method: default_str(method, "centroid").to_string(),
        k: parse_usize_default(k, 3, "k")?,
        similarity: default_str(similarity, "cosine").to_string(),
        weighting: default_str(weighting, "tfidf").to_string(),
        analyzer: default_str(analyzer, "word").to_string(),
        ngram_max: parse_usize_default(ngram_max, 1, "ngram_max")?,
        lowercase: truthy(lowercase, true),
        strip_accents: truthy(strip_accents, false),
        remove_stopwords: truthy(remove_stopwords, false),
        sublinear_tf: truthy(sublinear_tf, false),
        min_df: parse_usize_default(min_df, 1, "min_df")?,
        min_confidence: parse_f64_default(min_confidence, 0.0, "min_confidence")?,
        top_k: parse_usize_default(top_k, 3, "top_k")?,
        explain: truthy(explain, true),
        output: default_str(output, "report").to_string(),
    };
    gizza_ai_few_shot_text_classifier_core::classify(examples, text, &opts)
        .map_err(|e| JsValue::from_str(&e))
}

fn default_str<'a>(v: &'a str, default: &'a str) -> &'a str {
    if v.trim().is_empty() {
        default
    } else {
        v.trim()
    }
}

fn truthy(v: &str, default: bool) -> bool {
    let s = v.trim().to_ascii_lowercase();
    if s.is_empty() {
        default
    } else {
        matches!(s.as_str(), "true" | "1" | "on" | "yes")
    }
}

fn parse_usize_default(v: &str, default: usize, name: &str) -> Result<usize, JsValue> {
    if v.trim().is_empty() {
        Ok(default)
    } else {
        v.trim()
            .parse::<usize>()
            .map_err(|_| JsValue::from_str(&format!("{name} must be a whole number, got \"{v}\"")))
    }
}

fn parse_f64_default(v: &str, default: f64, name: &str) -> Result<f64, JsValue> {
    if v.trim().is_empty() {
        Ok(default)
    } else {
        v.trim()
            .parse::<f64>()
            .map_err(|_| JsValue::from_str(&format!("{name} must be a number, got \"{v}\"")))
    }
}
