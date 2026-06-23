//! Browser-facing wasm-bindgen wrapper for /tools/geometry-calculator/.
//!
//! Field order MUST match page/meta.toml: shape, then the dimension fields.
//! Each dimension arrives as a string; blank strings are treated as "unset"
//! (the shape only reads the dimensions it needs). A non-blank, non-numeric
//! dimension is a parse error.
use gizza_ai_geometry_calculator_core::Dimensions;
use wasm_bindgen::prelude::*;

/// Parse an optional numeric field: blank/whitespace → `None`; otherwise parse,
/// erroring on garbage.
fn parse_opt(label: &str, s: &str) -> Result<Option<f64>, String> {
    let t = s.trim();
    if t.is_empty() {
        return Ok(None);
    }
    t.parse::<f64>()
        .map(Some)
        .map_err(|_| format!("{label} must be a number (got '{t}')"))
}

/// Compute geometry for `shape` from the supplied dimension fields, returning a
/// pretty-printed JSON object. Throws the error string on failure.
#[allow(clippy::too_many_arguments)]
#[wasm_bindgen]
pub fn run(
    shape: &str,
    side: &str,
    width: &str,
    height: &str,
    length: &str,
    radius: &str,
    radius_a: &str,
    radius_b: &str,
    base: &str,
    top: &str,
    sides: &str,
    side_a: &str,
    side_b: &str,
    side_c: &str,
) -> Result<String, JsValue> {
    // Clean initial state: the shape <select> always has a value, so the page
    // can't detect an "empty" form the way single-field tools do. When the user
    // hasn't entered any dimension yet, show a prompt instead of a missing-input
    // error.
    if [
        side, width, height, length, radius, radius_a, radius_b, base, top, sides,
        side_a, side_b, side_c,
    ]
    .iter()
    .all(|s| s.trim().is_empty())
    {
        return Ok(format!("Enter the dimensions for a {shape} to see its measures."));
    }

    let dims = Dimensions {
        side: parse_opt("side", side).map_err(js)?,
        width: parse_opt("width", width).map_err(js)?,
        height: parse_opt("height", height).map_err(js)?,
        length: parse_opt("length", length).map_err(js)?,
        radius: parse_opt("radius", radius).map_err(js)?,
        radius_a: parse_opt("radius_a", radius_a).map_err(js)?,
        radius_b: parse_opt("radius_b", radius_b).map_err(js)?,
        base: parse_opt("base", base).map_err(js)?,
        top: parse_opt("top", top).map_err(js)?,
        sides: parse_opt("sides", sides).map_err(js)?,
        side_a: parse_opt("side_a", side_a).map_err(js)?,
        side_b: parse_opt("side_b", side_b).map_err(js)?,
        side_c: parse_opt("side_c", side_c).map_err(js)?,
    };
    gizza_ai_geometry_calculator_core::compute_json(shape, &dims).map_err(js)
}

fn js(e: String) -> JsValue {
    JsValue::from_str(&e)
}
