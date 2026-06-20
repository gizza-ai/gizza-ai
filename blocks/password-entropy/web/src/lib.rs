//! Browser-facing wasm-bindgen wrapper for /tools/password-entropy/.
//! Single field: password. Returns a human-readable summary.
use gizza_ai_password_entropy_core::analyze;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(password: &str) -> Result<String, JsValue> {
    let s = analyze(password).map_err(|e| JsValue::from_str(&e))?;
    let mut out = format!(
        "{} — {:.1} bits ({} chars, {}-symbol alphabet)\nEstimated crack time: {}",
        s.rating, s.bits, s.length, s.charset_size, s.crack_time
    );
    if !s.warnings.is_empty() {
        out.push_str("\nWarnings:");
        for w in &s.warnings {
            out.push_str(&format!("\n - {w}"));
        }
    }
    Ok(out)
}
