//! Browser-facing wasm-bindgen wrapper for /tools/recovery-code-generator/.
//! Field order MUST match meta.toml: count, blocks, chars_per_block, charset,
//! separator, output, hash, seed_hex. Every field arrives as a string; the
//! numeric ones are parsed here so a blank box falls back to the core default
//! rather than erroring.
use gizza_ai_recovery_code_generator_core::Options;
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

#[wasm_bindgen]
pub fn run(
    count: &str,
    blocks: &str,
    chars_per_block: &str,
    charset: &str,
    separator: &str,
    output: &str,
    hash: &str,
    seed_hex: &str,
) -> Result<String, JsValue> {
    let d = Options::default();
    let o = Options {
        count: num(count, "count", d.count).map_err(|e| JsValue::from_str(&e))?,
        blocks: num(blocks, "blocks", d.blocks).map_err(|e| JsValue::from_str(&e))?,
        chars_per_block: num(chars_per_block, "chars_per_block", d.chars_per_block)
            .map_err(|e| JsValue::from_str(&e))?,
        charset: charset.to_string(),
        separator: separator.to_string(),
        output: output.to_string(),
        hash: hash.to_string(),
        seed_hex: seed_hex.to_string(),
    };
    gizza_ai_recovery_code_generator_core::run(&o).map_err(|e| JsValue::from_str(&e))
}
