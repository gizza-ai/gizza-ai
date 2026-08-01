//! Browser-facing wasm-bindgen wrapper for /tools/pie-donut-chart-svg/.
//! The page passes every field as a string (in declared meta.toml order); this
//! parses the numeric/boolean options, builds the core Options, and returns the
//! SVG.
use gizza_ai_pie_donut_chart_svg_core::{render, Options};
use wasm_bindgen::prelude::*;

fn truthy(v: &str) -> bool {
    matches!(v.trim().to_lowercase().as_str(), "true" | "1" | "on" | "yes")
}

#[wasm_bindgen]
#[allow(clippy::too_many_arguments)]
pub fn run(
    data: &str,
    chart_type: &str,
    width: &str,
    height: &str,
    donut_hole: &str,
    start_angle: &str,
    colors: &str,
    show_labels: &str,
    show_percentages: &str,
    show_values: &str,
    legend: &str,
    sort: &str,
    title: &str,
    background: &str,
) -> Result<String, JsValue> {
    // Empty input → empty result (the page shows a neutral idle state rather
    // than a red error on first load / after Reset).
    if data.trim().is_empty() {
        return Ok(String::new());
    }
    let opts = Options {
        chart_type: if chart_type.trim().is_empty() { "pie".to_string() } else { chart_type.to_string() },
        width: width.trim().parse::<u32>().unwrap_or(640),
        height: height.trim().parse::<u32>().unwrap_or(400),
        donut_hole: donut_hole.trim().parse::<f64>().unwrap_or(0.55),
        start_angle: start_angle.trim().parse::<f64>().unwrap_or(0.0),
        colors: colors.to_string(),
        show_labels: truthy(show_labels),
        show_percentages: truthy(show_percentages),
        show_values: truthy(show_values),
        legend: if legend.trim().is_empty() { "right".to_string() } else { legend.to_string() },
        sort: if sort.trim().is_empty() { "input".to_string() } else { sort.to_string() },
        title: title.to_string(),
        background: if background.trim().is_empty() { "#ffffff".to_string() } else { background.to_string() },
    };
    render(data, &opts).map_err(|e| JsValue::from_str(&e))
}
