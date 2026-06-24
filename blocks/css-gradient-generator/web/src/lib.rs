//! Browser-facing wasm-bindgen wrapper for /tools/css-gradient-generator/.
//! The standalone page passes every field value as a string, so the numeric /
//! boolean params arrive as strings and are parsed here.
use wasm_bindgen::prelude::*;

/// Build a CSS gradient declaration.
///
/// - `colors`: comma/newline-separated CSS color stops.
/// - `gradient_type`: `"linear"` (blank → linear) / `"radial"` / `"conic"`.
/// - `angle`: degrees; blank/unparseable → NaN, which the core maps to the
///   per-type default (180 linear, 0 conic).
/// - `shape`: `"ellipse"` (blank → ellipse) / `"circle"` (radial only).
/// - `repeating`: `"true"`/`"1"`/`"yes"`/`"on"` → repeating gradient.
/// - `interpolation`: color-interpolation space (blank → none), e.g. `oklch`.
///
/// Throws a JS error string on invalid input.
#[wasm_bindgen]
pub fn run(
    colors: &str,
    gradient_type: &str,
    angle: &str,
    shape: &str,
    repeating: &str,
    interpolation: &str,
) -> Result<String, JsValue> {
    let angle = angle.trim().parse::<f64>().unwrap_or(f64::NAN);
    let repeating = matches!(
        repeating.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "yes" | "on"
    );
    gizza_ai_css_gradient_generator_core::generate(
        gradient_type,
        angle,
        shape,
        colors,
        repeating,
        interpolation,
    )
    .map_err(|e| JsValue::from_str(&e))
}
