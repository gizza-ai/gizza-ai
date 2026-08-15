//! Browser-facing wasm-bindgen wrapper for /tools/histogram-chart/.
use wasm_bindgen::prelude::*;

use gizza_ai_histogram_chart_core::Options;

fn boolish(s: &str) -> bool {
    matches!(s.trim().to_ascii_lowercase().as_str(), "true" | "1" | "on" | "yes")
}

fn text_or(s: &str, fallback: &str) -> String {
    if s.trim().is_empty() {
        fallback.into()
    } else {
        s.into()
    }
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

#[allow(clippy::too_many_arguments)]
#[wasm_bindgen]
pub fn run(
    data: &str,
    bin_method: &str,
    bins: &str,
    bin_width: &str,
    range_min: &str,
    range_max: &str,
    normalize: &str,
    right_closed: &str,
    show_values: &str,
    show_mean: &str,
    show_median: &str,
    normal_curve: &str,
    rug: &str,
    grid: &str,
    orientation: &str,
    title: &str,
    x_label: &str,
    y_label: &str,
    width: &str,
    height: &str,
    color: &str,
    opacity: &str,
    theme: &str,
    precision: &str,
    output: &str,
) -> Result<String, JsValue> {
    let opts = Options {
        bin_method: text_or(bin_method, "auto"),
        bins: parse_u32("bins", bins, 10)?,
        bin_width: parse_f64("bin_width", bin_width, 0.0)?,
        range_min: range_min.into(),
        range_max: range_max.into(),
        normalize: text_or(normalize, "count"),
        right_closed: boolish(right_closed),
        show_values: boolish(show_values),
        show_mean: boolish(show_mean),
        show_median: boolish(show_median),
        normal_curve: boolish(normal_curve),
        rug: boolish(rug),
        grid: boolish(grid),
        orientation: text_or(orientation, "vertical"),
        title: title.into(),
        x_label: x_label.into(),
        y_label: y_label.into(),
        width: parse_u32("width", width, 800)?,
        height: parse_u32("height", height, 480)?,
        color: text_or(color, "#2563eb"),
        opacity: parse_f64("opacity", opacity, 0.9)?,
        theme: text_or(theme, "light"),
        precision: parse_u32("precision", precision, 4)?,
        output: text_or(output, "svg"),
    };
    gizza_ai_histogram_chart_core::render(data, &opts).map_err(|e| JsValue::from_str(&e))
}
