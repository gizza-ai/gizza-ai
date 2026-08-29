//! Browser-facing wasm-bindgen wrapper for /tools/treemap-chart/.
use wasm_bindgen::prelude::*;

use gizza_ai_treemap_chart_core::Options;

fn boolish(s: &str) -> bool {
    matches!(s.trim().to_ascii_lowercase().as_str(), "true" | "1" | "on" | "yes")
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
    layout: &str,
    path_separator: &str,
    sort: &str,
    tiling: &str,
    max_depth: &str,
    top_n: &str,
    show_labels: &str,
    show_values: &str,
    show_percent: &str,
    label_position: &str,
    font_size: &str,
    palette: &str,
    color: &str,
    background: &str,
    border_width: &str,
    corner_radius: &str,
    title: &str,
    legend: &str,
    width: &str,
    height: &str,
    theme: &str,
    output: &str,
) -> Result<String, JsValue> {
    let opts = Options {
        layout: or_default(layout, "auto"),
        // An empty separator is meaningful only for flat/grouped rows; keep "/" as the default.
        path_separator: if path_separator.is_empty() {
            "/".to_string()
        } else {
            path_separator.to_string()
        },
        sort: or_default(sort, "value_desc"),
        tiling: or_default(tiling, "squarified"),
        max_depth: parse_u32("max_depth", max_depth, 0)?,
        top_n: parse_u32("top_n", top_n, 0)?,
        show_labels: boolish(show_labels),
        show_values: boolish(show_values),
        show_percent: boolish(show_percent),
        label_position: or_default(label_position, "top"),
        font_size: parse_f64("font_size", font_size, 13.0)?,
        palette: or_default(palette, "default"),
        color: or_default(color, "#2563eb"),
        background: background.trim().to_string(),
        border_width: parse_f64("border_width", border_width, 2.0)?,
        corner_radius: parse_f64("corner_radius", corner_radius, 2.0)?,
        title: title.to_string(),
        legend: boolish(legend),
        width: parse_u32("width", width, 800)?,
        height: parse_u32("height", height, 500)?,
        theme: or_default(theme, "light"),
        output: or_default(output, "svg"),
    };
    gizza_ai_treemap_chart_core::render(data, &opts).map_err(|e| JsValue::from_str(&e))
}
