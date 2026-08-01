//! Browser-facing wasm-bindgen wrapper for /tools/smart-quotes-converter/.
//!
//! tool.js passes EVERY page field as a raw string (no coercion for pure tools),
//! so this export takes `&str` for every param and parses the enum/bool fields
//! here; the core owns validation. Param order MUST match page/meta.toml's
//! [[input]] order: input, direction, convert_double, convert_single, feet_inches.
use gizza_ai_smart_quotes_converter_core::{convert, Direction, Options};
use wasm_bindgen::prelude::*;

fn truthy(s: &str, default: bool) -> bool {
    match s.trim().to_ascii_lowercase().as_str() {
        "" => default,
        "true" | "1" | "on" | "yes" => true,
        _ => false,
    }
}

#[wasm_bindgen]
pub fn run(
    input: &str,
    direction: &str,
    convert_double: &str,
    convert_single: &str,
    feet_inches: &str,
) -> Result<String, JsValue> {
    let opts = Options {
        direction: Direction::parse(direction),
        convert_double: truthy(convert_double, true),
        convert_single: truthy(convert_single, true),
        feet_inches: truthy(feet_inches, false),
    };
    convert(input, opts).map_err(|e| JsValue::from_str(&e))
}
