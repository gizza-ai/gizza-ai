//! Browser-facing wasm-bindgen wrapper for /tools/radar-chart/.
use wasm_bindgen::prelude::*;

use gizza_ai_radar_chart_core::Options;

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
    layout: &str,
    scale: &str,
    scale_min: &str,
    scale_max: &str,
    rings: &str,
    grid_shape: &str,
    show_spokes: &str,
    show_axis_labels: &str,
    show_ticks: &str,
    show_values: &str,
    fill_opacity: &str,
    line_width: &str,
    point_radius: &str,
    start_angle: &str,
    direction: &str,
    palette: &str,
    colors: &str,
    background: &str,
    title: &str,
    legend: &str,
    font_size: &str,
    width: &str,
    height: &str,
    theme: &str,
    output: &str,
) -> Result<String, JsValue> {
    let opts = Options {
        layout: or_default(layout, "auto"),
        scale: or_default(scale, "shared"),
        scale_min: parse_f64("scale_min", scale_min, 0.0)?,
        scale_max: parse_f64("scale_max", scale_max, 0.0)?,
        rings: parse_u32("rings", rings, 5)?,
        grid_shape: or_default(grid_shape, "polygon"),
        show_spokes: boolish(show_spokes),
        show_axis_labels: boolish(show_axis_labels),
        show_ticks: boolish(show_ticks),
        show_values: boolish(show_values),
        fill_opacity: parse_f64("fill_opacity", fill_opacity, 0.25)?,
        line_width: parse_f64("line_width", line_width, 2.0)?,
        point_radius: parse_f64("point_radius", point_radius, 3.0)?,
        start_angle: parse_f64("start_angle", start_angle, 0.0)?,
        direction: or_default(direction, "clockwise"),
        palette: or_default(palette, "default"),
        // Colour fields stay verbatim: a comma list is meaningful and `transparent` is valid.
        colors: colors.trim().to_string(),
        background: background.trim().to_string(),
        title: title.to_string(),
        legend: boolish(legend),
        font_size: parse_f64("font_size", font_size, 13.0)?,
        width: parse_u32("width", width, 700)?,
        height: parse_u32("height", height, 560)?,
        theme: or_default(theme, "light"),
        output: or_default(output, "svg"),
    };
    gizza_ai_radar_chart_core::render(data, &opts).map_err(|e| JsValue::from_str(&e))
}
