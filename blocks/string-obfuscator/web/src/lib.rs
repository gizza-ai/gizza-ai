//! Browser-facing wasm-bindgen wrapper for /tools/string-obfuscator/.
//! Compiled with wasm-pack for the standalone page.
use wasm_bindgen::prelude::*;

/// Mask or obfuscate `text`.
///
/// The standalone tool page passes every field value as a string, so the
/// integer params arrive as strings and are parsed here (blank/unparseable →
/// the documented default).
/// - `mode`: `"mask"` (default) / `"rot"` / `"leetspeak"` / `"homoglyph"`.
/// - `mask_char`: replacement char for `mask` mode (blank → `*`).
/// - `keep_start` / `keep_end`: chars to keep visible in `mask` mode (→ 0).
/// - `rot_n`: ROT shift for `rot` mode (blank → 13).
///
/// Throws a JS error string on an invalid `mode`.
#[wasm_bindgen]
pub fn run(
    text: &str,
    mode: &str,
    mask_char: &str,
    keep_start: &str,
    keep_end: &str,
    rot_n: &str,
) -> Result<String, JsValue> {
    let keep_start = keep_start.trim().parse::<usize>().unwrap_or(0);
    let keep_end = keep_end.trim().parse::<usize>().unwrap_or(0);
    let rot_n = rot_n.trim().parse::<u32>().unwrap_or(13);
    gizza_ai_string_obfuscator_core::obfuscate(text, mode, mask_char, keep_start, keep_end, rot_n)
        .map_err(|e| JsValue::from_str(&e))
}
