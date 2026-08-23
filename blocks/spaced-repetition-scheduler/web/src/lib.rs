//! Browser-facing wasm-bindgen wrapper for /tools/spaced-repetition-scheduler/.
//! Argument order mirrors the descriptor's param order (and page/meta.toml's [[input]] order).
use wasm_bindgen::prelude::*;

fn num(v: &str, default: f64, name: &str) -> Result<f64, JsValue> {
    if v.trim().is_empty() {
        Ok(default)
    } else {
        v.trim()
            .parse::<f64>()
            .map_err(|_| JsValue::from_str(&format!("{name} must be a number")))
    }
}

fn int(v: &str, default: i64, name: &str) -> Result<i64, JsValue> {
    if v.trim().is_empty() {
        Ok(default)
    } else {
        v.trim()
            .parse::<f64>()
            .ok()
            .filter(|n| n.is_finite() && n.fract() == 0.0)
            .map(|n| n as i64)
            .ok_or_else(|| JsValue::from_str(&format!("{name} must be a whole number")))
    }
}

fn or<'a>(v: &'a str, default: &'a str) -> &'a str {
    if v.trim().is_empty() {
        default
    } else {
        v
    }
}

/// `true` unless the checkbox came through as an explicit falsey value.
fn flag(v: &str) -> bool {
    matches!(
        v.trim().to_ascii_lowercase().as_str(),
        "1" | "true" | "yes" | "on"
    )
}

#[allow(clippy::too_many_arguments)]
#[wasm_bindgen]
pub fn run(
    reviews: &str,
    algorithm: &str,
    grade_scale: &str,
    today: &str,
    output: &str,
    sort: &str,
    only_due: &str,
    desired_retention: &str,
    ease_start: &str,
    min_ease: &str,
    first_interval: &str,
    second_interval: &str,
    easy_bonus: &str,
    hard_multiplier: &str,
    interval_modifier: &str,
    lapse_multiplier: &str,
    max_interval: &str,
    leech_threshold: &str,
    forecast_reviews: &str,
    forecast_grade: &str,
    fsrs_weights: &str,
) -> Result<String, JsValue> {
    gizza_ai_spaced_repetition_scheduler_core::schedule(
        reviews,
        or(algorithm, "sm2"),
        or(grade_scale, "auto"),
        today,
        or(output, "table"),
        or(sort, "due"),
        flag(only_due),
        num(desired_retention, 0.9, "desired_retention")?,
        num(ease_start, 2.5, "ease_start")?,
        num(min_ease, 1.3, "min_ease")?,
        num(first_interval, 1.0, "first_interval")?,
        num(second_interval, 6.0, "second_interval")?,
        num(easy_bonus, 1.3, "easy_bonus")?,
        num(hard_multiplier, 1.2, "hard_multiplier")?,
        num(interval_modifier, 1.0, "interval_modifier")?,
        num(lapse_multiplier, 0.0, "lapse_multiplier")?,
        num(max_interval, 36500.0, "max_interval")?,
        int(leech_threshold, 8, "leech_threshold")?,
        int(forecast_reviews, 6, "forecast_reviews")?,
        or(forecast_grade, "good"),
        fsrs_weights,
    )
    .map_err(|e| JsValue::from_str(&e))
}
