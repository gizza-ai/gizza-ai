//! Browser-facing wasm-bindgen wrapper for /tools/color-palette-generator/.
use gizza_ai_color_palette_generator_core::generate;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(color: &str, scheme: &str, count: &str) -> Result<String, JsValue> {
    let n: usize = count.trim().parse().unwrap_or(5);
    let p = generate(color, scheme, n).map_err(|e| JsValue::from_str(&e))?;
    let mut out = format!("{} palette from {}\n", p.scheme, p.base);
    for (i, c) in p.colors.iter().enumerate() {
        out.push_str(&format!(
            "\n{}. {}\n   {}\n   {}",
            i + 1,
            c.hex,
            c.rgb,
            c.hsl
        ));
    }
    Ok(out)
}
