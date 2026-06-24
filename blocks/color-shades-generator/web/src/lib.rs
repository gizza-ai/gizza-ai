//! Browser-facing wasm-bindgen wrapper for /tools/color-shades-generator/.
use gizza_ai_color_shades_generator_core::generate;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(color: &str, mode: &str, count: &str) -> Result<String, JsValue> {
    let n: usize = count.trim().parse().unwrap_or(9);
    let r = generate(color, mode, n).map_err(|e| JsValue::from_str(&e))?;
    let mut out = format!("{} ramp from {}\n", r.mode, r.base);
    for s in &r.steps {
        let marker = if s.is_base { "  (base)" } else { "" };
        out.push_str(&format!(
            "\n{}: {}{}\n   {}\n   {}",
            s.name, s.hex, marker, s.rgb, s.hsl
        ));
    }
    Ok(out)
}
