//! Browser-facing wasm-bindgen wrapper for /tools/time-series-generator/.
use wasm_bindgen::prelude::*;

fn parse_usize(v: &str, default: usize, name: &str) -> Result<usize, JsValue> {
    let t = v.trim();
    if t.is_empty() {
        Ok(default)
    } else {
        t.parse::<usize>()
            .map_err(|_| JsValue::from_str(&format!("{name} must be a whole number")))
    }
}

fn parse_u64(v: &str, default: u64, name: &str) -> Result<u64, JsValue> {
    let t = v.trim();
    if t.is_empty() {
        Ok(default)
    } else {
        t.parse::<u64>()
            .map_err(|_| JsValue::from_str(&format!("{name} must be a non-negative whole number")))
    }
}

fn parse_f64(v: &str, default: f64, name: &str) -> Result<f64, JsValue> {
    let t = v.trim();
    if t.is_empty() {
        Ok(default)
    } else {
        t.parse::<f64>()
            .map_err(|_| JsValue::from_str(&format!("{name} must be a number")))
    }
}

fn truthy(v: &str) -> bool {
    matches!(
        v.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "yes" | "on"
    )
}

#[allow(clippy::too_many_arguments)]
#[wasm_bindgen]
pub fn run(
    start: &str,
    interval: &str,
    count: &str,
    base: &str,
    trend: &str,
    trend_strength: &str,
    seasonality: &str,
    period: &str,
    amplitude: &str,
    weekday_pattern: &str,
    combine: &str,
    noise: &str,
    noise_level: &str,
    noise_phi: &str,
    missing_rate: &str,
    outlier_rate: &str,
    outlier_magnitude: &str,
    outlier_direction: &str,
    min_value: &str,
    max_value: &str,
    series: &str,
    seed: &str,
    decimals: &str,
    output: &str,
    timestamp_format: &str,
    header: &str,
    labels: &str,
) -> Result<String, JsValue> {
    let spec = gizza_ai_time_series_generator_core::Spec {
        start: if start.trim().is_empty() {
            "2024-01-01"
        } else {
            start
        },
        interval: if interval.trim().is_empty() {
            "1d"
        } else {
            interval
        },
        count: parse_usize(count, 100, "count")?,
        base: parse_f64(base, 100.0, "base")?,
        trend: if trend.trim().is_empty() {
            "linear"
        } else {
            trend
        },
        trend_strength: parse_f64(trend_strength, 0.5, "trend_strength")?,
        seasonality: if seasonality.trim().is_empty() {
            "sine"
        } else {
            seasonality
        },
        period: if period.trim().is_empty() {
            "7"
        } else {
            period
        },
        amplitude: if amplitude.trim().is_empty() {
            "10"
        } else {
            amplitude
        },
        weekday_pattern: if weekday_pattern.trim().is_empty() {
            "1.1, 1.05, 1, 1.05, 1.25, 0.8, 0.7"
        } else {
            weekday_pattern
        },
        combine: if combine.trim().is_empty() {
            "additive"
        } else {
            combine
        },
        noise: if noise.trim().is_empty() {
            "gaussian"
        } else {
            noise
        },
        noise_level: parse_f64(noise_level, 5.0, "noise_level")?,
        noise_phi: parse_f64(noise_phi, 0.7, "noise_phi")?,
        missing_rate: parse_f64(missing_rate, 0.0, "missing_rate")?,
        outlier_rate: parse_f64(outlier_rate, 0.0, "outlier_rate")?,
        outlier_magnitude: parse_f64(outlier_magnitude, 3.0, "outlier_magnitude")?,
        outlier_direction: if outlier_direction.trim().is_empty() {
            "both"
        } else {
            outlier_direction
        },
        min_value,
        max_value,
        series: parse_usize(series, 1, "series")?,
        seed: parse_u64(seed, 42, "seed")?,
        decimals: parse_usize(decimals, 2, "decimals")?,
        output: if output.trim().is_empty() {
            "csv"
        } else {
            output
        },
        timestamp_format: if timestamp_format.trim().is_empty() {
            "auto"
        } else {
            timestamp_format
        },
        header: truthy(header),
        labels,
    };
    gizza_ai_time_series_generator_core::generate(&spec).map_err(|e| JsValue::from_str(&e))
}
