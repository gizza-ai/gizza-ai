//! Browser-facing wasm-bindgen wrapper for /tools/pca-visualizer/.
//! The page passes every field as a string, so numbers/booleans are parsed here
//! and all validation stays in the shared core.
use wasm_bindgen::prelude::*;

/// Checkboxes arrive as `"true"`/`"false"`; treat every positive spelling as on.
fn truthy(v: &str, default: bool) -> bool {
    let s = v.trim().to_ascii_lowercase();
    if s.is_empty() {
        default
    } else {
        matches!(s.as_str(), "true" | "1" | "on" | "yes")
    }
}

fn number(v: &str, what: &str, default: f64) -> Result<f64, JsValue> {
    let s = v.trim();
    if s.is_empty() {
        return Ok(default);
    }
    s.parse::<f64>()
        .map_err(|_| JsValue::from_str(&format!("{what} must be a number, got '{s}'")))
}

#[allow(clippy::too_many_arguments)]
#[wasm_bindgen]
pub fn run(
    data: &str,
    method: &str,
    label_column: &str,
    scale: &str,
    perplexity: &str,
    iterations: &str,
    learning_rate: &str,
    show_labels: &str,
    point_size: &str,
    title: &str,
    width: &str,
    height: &str,
    format: &str,
) -> Result<String, JsValue> {
    let perplexity = number(perplexity, "perplexity", 30.0)?;
    let iterations = number(iterations, "iterations", 500.0)?;
    let learning_rate = number(learning_rate, "learning_rate", 200.0)?;
    let point_size = number(point_size, "point_size", 4.0)?;
    let width = number(width, "width", 720.0)?;
    let height = number(height, "height", 520.0)?;
    gizza_ai_pca_visualizer_core::run(
        data,
        method,
        label_column,
        truthy(scale, true),
        perplexity,
        iterations,
        learning_rate,
        truthy(show_labels, false),
        point_size,
        title,
        width,
        height,
        format,
    )
    .map_err(|e| JsValue::from_str(&e))
}
