//! Browser-facing wasm-bindgen wrapper for /tools/pareto-chart/.
//! The page driver hands every field over as a raw string, so each value is parsed here
//! and the core owns all clamping/validation — one shared code path for chat, CLI, page.
use wasm_bindgen::prelude::*;

use gizza_ai_pareto_chart_core::Options;

fn boolish(s: &str) -> bool {
    matches!(
        s.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "on" | "yes"
    )
}

fn or_default(s: &str, fallback: &str) -> String {
    if s.trim().is_empty() {
        fallback.to_string()
    } else {
        s.trim().to_string()
    }
}

fn parse_f64(name: &str, s: &str, fallback: f64) -> Result<f64, JsValue> {
    let t = s.trim();
    if t.is_empty() {
        Ok(fallback)
    } else {
        t.parse::<f64>()
            .map_err(|_| JsValue::from_str(&format!("{name} must be a number, got `{t}`")))
    }
}

fn parse_u32(name: &str, s: &str, fallback: u32) -> Result<u32, JsValue> {
    let t = s.trim();
    if t.is_empty() {
        Ok(fallback)
    } else {
        t.parse::<u32>()
            .map_err(|_| JsValue::from_str(&format!("{name} must be a whole number, got `{t}`")))
    }
}

#[allow(clippy::too_many_arguments)]
#[wasm_bindgen]
pub fn run(
    data: &str,
    delimiter: &str,
    header: &str,
    sort: &str,
    max_categories: &str,
    other_label: &str,
    threshold: &str,
    highlight_vital_few: &str,
    show_cumulative: &str,
    show_values: &str,
    show_cumulative_labels: &str,
    decimals: &str,
    title: &str,
    category_label: &str,
    value_label: &str,
    percent_label: &str,
    label_angle: &str,
    color: &str,
    vital_color: &str,
    line_color: &str,
    threshold_color: &str,
    background: &str,
    bar_width: &str,
    line_width: &str,
    point_radius: &str,
    grid: &str,
    legend: &str,
    font_size: &str,
    width: &str,
    height: &str,
    theme: &str,
    output: &str,
) -> Result<String, JsValue> {
    let opts = Options {
        delimiter: or_default(delimiter, "auto"),
        header: or_default(header, "auto"),
        sort: or_default(sort, "desc"),
        max_categories: parse_u32("max_categories", max_categories, 0)?,
        other_label: or_default(other_label, "Other"),
        threshold: parse_f64("threshold", threshold, 80.0)?,
        highlight_vital_few: boolish(highlight_vital_few),
        show_cumulative: boolish(show_cumulative),
        show_values: boolish(show_values),
        show_cumulative_labels: boolish(show_cumulative_labels),
        decimals: parse_u32("decimals", decimals, 1)?,
        title: title.trim().to_string(),
        category_label: category_label.trim().to_string(),
        value_label: value_label.trim().to_string(),
        // Blank is meaningful here: it drops the right-hand axis title.
        percent_label: percent_label.trim().to_string(),
        label_angle: parse_f64("label_angle", label_angle, 0.0)?,
        // Colour fields stay verbatim: named colours and `transparent` are valid.
        color: or_default(color, "#2563eb"),
        vital_color: or_default(vital_color, "#f97316"),
        line_color: or_default(line_color, "#dc2626"),
        threshold_color: or_default(threshold_color, "#94a3b8"),
        background: background.trim().to_string(),
        bar_width: parse_f64("bar_width", bar_width, 0.8)?,
        line_width: parse_f64("line_width", line_width, 2.0)?,
        point_radius: parse_f64("point_radius", point_radius, 3.5)?,
        grid: boolish(grid),
        legend: boolish(legend),
        font_size: parse_f64("font_size", font_size, 13.0)?,
        width: parse_u32("width", width, 820)?,
        height: parse_u32("height", height, 520)?,
        theme: or_default(theme, "light"),
        output: or_default(output, "svg"),
    };
    gizza_ai_pareto_chart_core::render(data, &opts).map_err(|e| JsValue::from_str(&e))
}
