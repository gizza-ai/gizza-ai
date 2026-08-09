//! Browser-facing wasm-bindgen wrapper for /tools/boxplot-chart/.
use wasm_bindgen::prelude::*;

use gizza_ai_boxplot_chart_core::Options;

fn boolish(s: &str) -> bool {
    matches!(s.trim().to_ascii_lowercase().as_str(), "true" | "1" | "on" | "yes")
}

fn parse_f64(name: &str, s: &str, fallback: f64) -> Result<f64, JsValue> {
    let t = s.trim();
    if t.is_empty() {
        Ok(fallback)
    } else {
        t.parse::<f64>()
            .map_err(|_| JsValue::from_str(&format!("{name} must be a number")))
    }
}

fn parse_u32(name: &str, s: &str, fallback: u32) -> Result<u32, JsValue> {
    let t = s.trim();
    if t.is_empty() {
        Ok(fallback)
    } else {
        t.parse::<u32>()
            .map_err(|_| JsValue::from_str(&format!("{name} must be an integer")))
    }
}

#[wasm_bindgen]
pub fn run(
    data: &str,
    layout: &str,
    group_column: &str,
    value_column: &str,
    quartile_method: &str,
    whiskers: &str,
    iqr_multiplier: &str,
    percentile: &str,
    points: &str,
    show_mean: &str,
    notched: &str,
    orientation: &str,
    grid: &str,
    title: &str,
    value_label: &str,
    group_label: &str,
    width: &str,
    height: &str,
    color: &str,
    theme: &str,
    output: &str,
) -> Result<String, JsValue> {
    let opts = Options {
        layout: if layout.trim().is_empty() { "auto".into() } else { layout.into() },
        group_column: group_column.into(),
        value_column: value_column.into(),
        quartile_method: if quartile_method.trim().is_empty() { "linear".into() } else { quartile_method.into() },
        whiskers: if whiskers.trim().is_empty() { "tukey".into() } else { whiskers.into() },
        iqr_multiplier: parse_f64("iqr_multiplier", iqr_multiplier, 1.5)?,
        percentile: parse_f64("percentile", percentile, 5.0)?,
        points: if points.trim().is_empty() { "outliers".into() } else { points.into() },
        show_mean: boolish(show_mean),
        notched: boolish(notched),
        orientation: if orientation.trim().is_empty() { "vertical".into() } else { orientation.into() },
        grid: boolish(grid),
        title: title.into(),
        value_label: value_label.into(),
        group_label: group_label.into(),
        width: parse_u32("width", width, 800)?,
        height: parse_u32("height", height, 480)?,
        color: if color.trim().is_empty() { "#2563eb".into() } else { color.into() },
        theme: if theme.trim().is_empty() { "light".into() } else { theme.into() },
        output: if output.trim().is_empty() { "svg".into() } else { output.into() },
    };
    gizza_ai_boxplot_chart_core::render(data, &opts).map_err(|e| JsValue::from_str(&e))
}
