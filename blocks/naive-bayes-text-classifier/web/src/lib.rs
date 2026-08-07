//! Browser-facing wasm-bindgen wrapper for /tools/naive-bayes-text-classifier/.
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
#[allow(clippy::too_many_arguments)]
pub fn run(
    training_data: &str,
    text: &str,
    separator: &str,
    input_mode: &str,
    model: &str,
    alpha: &str,
    ngram_max: &str,
    lowercase: &str,
    remove_stopwords: &str,
    min_count: &str,
    priors: &str,
    top_k: &str,
    explain: &str,
    output: &str,
) -> Result<String, JsValue> {
    let opts = gizza_ai_naive_bayes_text_classifier_core::Options {
        separator: default_str(separator, "auto").to_string(),
        input_mode: default_str(input_mode, "single").to_string(),
        model: default_str(model, "multinomial").to_string(),
        alpha: parse_f64_default(alpha, 1.0, "alpha")?,
        ngram_max: parse_usize_default(ngram_max, 1, "ngram_max")?,
        lowercase: truthy(lowercase, true),
        remove_stopwords: truthy(remove_stopwords, false),
        min_count: parse_usize_default(min_count, 1, "min_count")?,
        priors: default_str(priors, "empirical").to_string(),
        top_k: parse_usize_default(top_k, 3, "top_k")?,
        explain: truthy(explain, true),
        output: default_str(output, "report").to_string(),
    };
    gizza_ai_naive_bayes_text_classifier_core::classify(training_data, text, &opts)
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
