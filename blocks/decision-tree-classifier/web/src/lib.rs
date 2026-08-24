//! Browser-facing wasm-bindgen wrapper for /tools/decision-tree-classifier/.
use wasm_bindgen::prelude::*;

#[allow(clippy::too_many_arguments)]
#[wasm_bindgen]
pub fn run(
    data: &str,
    target: &str,
    features: &str,
    criterion: &str,
    splits: &str,
    max_depth: &str,
    min_samples_split: &str,
    min_samples_leaf: &str,
    min_gain: &str,
    class_weight: &str,
    test_split: &str,
    seed: &str,
    predict: &str,
    header: &str,
    decimals: &str,
    format: &str,
) -> Result<String, JsValue> {
    let d = gizza_ai_decision_tree_classifier_core::Options::default();
    let opts = gizza_ai_decision_tree_classifier_core::Options {
        target: value_or(target, &d.target),
        features: features.to_string(),
        criterion: value_or(criterion, &d.criterion),
        splits: value_or(splits, &d.splits),
        max_depth: parse_u(max_depth, d.max_depth)?,
        min_samples_split: parse_u(min_samples_split, d.min_samples_split)?,
        min_samples_leaf: parse_u(min_samples_leaf, d.min_samples_leaf)?,
        min_gain: parse_f(min_gain, d.min_gain)?,
        class_weight: value_or(class_weight, &d.class_weight),
        test_split: parse_f(test_split, d.test_split)?,
        seed: parse_u64(seed, d.seed)?,
        predict: predict.to_string(),
        header: value_or(header, &d.header),
        decimals: parse_u(decimals, d.decimals)?,
        format: value_or(format, &d.format),
    };
    gizza_ai_decision_tree_classifier_core::run(data, &opts).map_err(|e| JsValue::from_str(&e))
}

fn value_or(v: &str, d: &str) -> String {
    if v.trim().is_empty() {
        d.to_string()
    } else {
        v.to_string()
    }
}
fn parse_f(v: &str, d: f64) -> Result<f64, JsValue> {
    if v.trim().is_empty() {
        Ok(d)
    } else {
        v.trim()
            .parse()
            .map_err(|_| JsValue::from_str("expected a number"))
    }
}
fn parse_u(v: &str, d: u32) -> Result<u32, JsValue> {
    if v.trim().is_empty() {
        Ok(d)
    } else {
        v.trim()
            .parse()
            .map_err(|_| JsValue::from_str("expected an integer"))
    }
}
fn parse_u64(v: &str, d: u64) -> Result<u64, JsValue> {
    if v.trim().is_empty() {
        Ok(d)
    } else {
        v.trim()
            .parse()
            .map_err(|_| JsValue::from_str("expected an integer"))
    }
}
