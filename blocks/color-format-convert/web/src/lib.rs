//! Browser-facing wasm-bindgen wrapper for /tools/color-format-convert/.
use gizza_ai_color_format_convert_core::convert;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(color: &str) -> Result<String, JsValue> {
    let c = convert(color).map_err(|e| JsValue::from_str(&e))?;
    Ok(format!(
        "HEX:  {}\nRGB:  {}\nRGBA: {}\nHSL:  {}\nHSV:  {}\nCMYK: {}",
        c.hex, c.rgb, c.rgba, c.hsl, c.hsv, c.cmyk
    ))
}
