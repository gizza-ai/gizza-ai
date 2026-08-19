//! Browser-facing wasm-bindgen wrapper for /tools/log-pattern-miner/.
//! Compiled with wasm-pack for the standalone page.
use wasm_bindgen::prelude::*;

/// Mine `logs` into ranked message templates and render the result.
///
/// The standalone tool page passes every field value as a string, so the
/// numeric params arrive as strings and are parsed here, each falling back to
/// the descriptor's default when the field is left blank:
/// - `format`: `"table"` (default) / `"json"` / `"lines"`.
/// - `similarity`: 0–1, default `0.4` (rendered as a slider).
/// - `depth`: 2–8, default `4` (slider).
/// - `max_children`: 2–1000, default `100`.
/// - `max_patterns`: 1–500, default `20`.
/// - `min_count`: ≥1, default `1`.
/// - `mask`: `"typed"` (default) / `"wildcard"` / `"none"`.
/// - `extra_delimiters`: free text, blank → whitespace-only tokenizing.
/// - `skip_tokens`: 0–16, default `0`.
///
/// Throws a JS error string on a non-numeric number field, an invalid
/// `format`/`mask`, an out-of-range knob, empty or over-long input, or when no
/// template reaches `min_count`.
#[wasm_bindgen]
#[allow(clippy::too_many_arguments)]
pub fn run(
    logs: &str,
    format: &str,
    similarity: &str,
    depth: &str,
    max_children: &str,
    max_patterns: &str,
    min_count: &str,
    mask: &str,
    extra_delimiters: &str,
    skip_tokens: &str,
) -> Result<String, JsValue> {
    let similarity = parse_f64("similarity", similarity, 0.4).map_err(err)?;
    let depth = parse_u32("depth", depth, 4).map_err(err)?;
    let max_children = parse_u32("max_children", max_children, 100).map_err(err)?;
    let max_patterns = parse_u32("max_patterns", max_patterns, 20).map_err(err)?;
    let min_count = parse_u32("min_count", min_count, 1).map_err(err)?;
    let skip_tokens = parse_u32("skip_tokens", skip_tokens, 0).map_err(err)?;
    // Blank selects fall through to the core's own "" → default handling.
    gizza_ai_log_pattern_miner_core::mine(
        logs,
        format,
        similarity,
        depth,
        max_children,
        max_patterns,
        min_count,
        mask,
        extra_delimiters,
        skip_tokens,
    )
    .map_err(err)
}

fn err(e: String) -> JsValue {
    JsValue::from_str(&e)
}

/// A blank field means "use the default"; anything unparseable is a hard error
/// rather than a silently substituted value.
fn parse_f64(name: &str, v: &str, default: f64) -> Result<f64, String> {
    match v.trim() {
        "" => Ok(default),
        s => s
            .parse::<f64>()
            .map_err(|_| format!("invalid {name} {s:?}: expected a number")),
    }
}

fn parse_u32(name: &str, v: &str, default: u32) -> Result<u32, String> {
    match v.trim() {
        "" => Ok(default),
        s => s
            .parse::<u32>()
            .map_err(|_| format!("invalid {name} {s:?}: expected a whole number")),
    }
}
