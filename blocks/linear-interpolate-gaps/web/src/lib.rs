//! Browser-facing wasm-bindgen wrapper for /tools/linear-interpolate-gaps/.
//! Field order MUST match page/meta.toml: input, layout, max_gap, direction,
//! edges, na_tokens, decimals, output. Every field arrives as a string (number
//! boxes and sliders send text); the core owns all validation and error messages.
use wasm_bindgen::prelude::*;
use gizza_ai_linear_interpolate_gaps_core::{interpolate_gaps, Options};

/// Blank / unparseable falls back to the descriptor default, then clamps to the
/// advertised range — the page must never fail on an empty number box.
fn num_of(s: &str, default: usize, max: usize) -> usize {
    let v = match s.trim() {
        "" => default,
        t => t.parse::<i64>().unwrap_or(default as i64).max(0) as usize,
    };
    v.min(max)
}

/// Fill the gaps in an ordered numeric series by linear interpolation.
///
/// - `input`: the series text — one value per line or comma/semicolon/tab/space
///   separated; blanks and NA markers are the gaps. Two-field rows read as `x,y`.
/// - `layout`: `auto` | `values` | `xy`.
/// - `max_gap`: longest fillable run of blanks, `0` = no limit.
/// - `direction`: `both` | `forward` | `backward`.
/// - `edges`: `leave` | `hold` | `extrapolate` for gaps outside the known range.
/// - `na_tokens`: extra comma-separated missing markers.
/// - `decimals`: rounding for computed values, `0`-`12`.
/// - `output`: `values` | `csv` | `json`.
///
/// Throws a JS error string on an unparseable series or an unknown option.
#[allow(clippy::too_many_arguments)]
#[wasm_bindgen]
pub fn run(
    input: &str,
    layout: &str,
    max_gap: &str,
    direction: &str,
    edges: &str,
    na_tokens: &str,
    decimals: &str,
    output: &str,
) -> Result<String, JsValue> {
    let opts = Options {
        layout: layout.to_string(),
        max_gap: num_of(max_gap, 0, 100_000),
        direction: direction.to_string(),
        edges: edges.to_string(),
        na_tokens: na_tokens.to_string(),
        decimals: num_of(decimals, 6, 12),
        output: output.to_string(),
    };
    interpolate_gaps(input, &opts).map_err(|e| JsValue::from_str(&e))
}
