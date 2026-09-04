//! Browser-facing wasm-bindgen wrapper for /tools/regex-match-generator/.
use wasm_bindgen::prelude::*;

fn parse_usize(name: &str, value: &str, default: usize) -> Result<usize, JsValue> {
    let v = value.trim();
    if v.is_empty() {
        return Ok(default);
    }
    v.parse::<usize>()
        .map_err(|_| JsValue::from_str(&format!("{name} must be a whole number")))
}

fn parse_u64(name: &str, value: &str, default: u64) -> Result<u64, JsValue> {
    let v = value.trim();
    if v.is_empty() {
        return Ok(default);
    }
    v.parse::<u64>()
        .map_err(|_| JsValue::from_str(&format!("{name} must be a whole number")))
}

fn parse_u32(name: &str, value: &str, default: u32) -> Result<u32, JsValue> {
    let v = value.trim();
    if v.is_empty() {
        return Ok(default);
    }
    v.parse::<u32>()
        .map_err(|_| JsValue::from_str(&format!("{name} must be a whole number")))
}

fn truthy(value: &str, default: bool) -> bool {
    match value.trim().to_ascii_lowercase().as_str() {
        "" => default,
        "true" | "1" | "on" | "yes" => true,
        _ => false,
    }
}

fn or_default<'a>(value: &'a str, default: &'a str) -> &'a str {
    if value.trim().is_empty() {
        default
    } else {
        value.trim()
    }
}

#[wasm_bindgen]
pub fn run(
    pattern: &str,
    count: &str,
    style: &str,
    seed: &str,
    max_repeat: &str,
    max_length: &str,
    unique: &str,
    output: &str,
) -> Result<String, JsValue> {
    gizza_ai_regex_match_generator_core::run(
        pattern,
        parse_usize("count", count, 5)?,
        or_default(style, "random"),
        parse_u64("seed", seed, 42)?,
        parse_u32("max_repeat", max_repeat, 4)?,
        parse_usize("max_length", max_length, 200)?,
        truthy(unique, true),
        or_default(output, "lines"),
    )
    .map_err(|e| JsValue::from_str(&e))
}
