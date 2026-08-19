//! Browser-facing wasm-bindgen wrapper for /tools/random-bytes/.
//! Field order MUST match meta.toml: bytes, count, encoding, separator,
//! uppercase, output, seed_hex. Every field arrives as a string; the numeric
//! ones are parsed here so a blank box falls back to the core default rather
//! than erroring, and the checkbox is read positive-truthy.
use gizza_ai_random_bytes_core::Options;
use wasm_bindgen::prelude::*;

/// Parse a number field, treating blank as "use the default". A non-numeric
/// value is reported by name instead of being silently defaulted.
fn num(v: &str, name: &str, fallback: usize) -> Result<usize, String> {
    let t = v.trim();
    if t.is_empty() {
        return Ok(fallback);
    }
    t.parse::<usize>()
        .map_err(|_| format!("{name} must be a whole number, got {t:?}"))
}

/// The page sends checkboxes as "true"/"false"; accept the other positive forms
/// a caller might send too.
fn flag(v: &str) -> bool {
    matches!(v.trim().to_ascii_lowercase().as_str(), "true" | "1" | "on" | "yes")
}

#[wasm_bindgen]
pub fn run(
    bytes: &str,
    count: &str,
    encoding: &str,
    separator: &str,
    uppercase: &str,
    output: &str,
    seed_hex: &str,
) -> Result<String, JsValue> {
    let d = Options::default();
    let o = Options {
        bytes: num(bytes, "bytes", d.bytes).map_err(|e| JsValue::from_str(&e))?,
        count: num(count, "count", d.count).map_err(|e| JsValue::from_str(&e))?,
        encoding: encoding.to_string(),
        separator: separator.to_string(),
        uppercase: flag(uppercase),
        output: output.to_string(),
        seed_hex: seed_hex.to_string(),
    };
    gizza_ai_random_bytes_core::run(&o).map_err(|e| JsValue::from_str(&e))
}
