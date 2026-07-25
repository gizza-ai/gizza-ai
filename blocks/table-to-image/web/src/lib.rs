//! Browser-facing wasm-bindgen wrapper for /tools/table-to-image/.
use gizza_ai_table_to_image_core::{render, Options};
use wasm_bindgen::prelude::*;

fn parse_bool(s: &str, default: bool) -> bool {
    match s.trim().to_ascii_lowercase().as_str() {
        "true" | "1" | "yes" | "on" => true,
        "false" | "0" | "no" | "off" => false,
        _ => default,
    }
}

#[wasm_bindgen]
pub fn run(
    input: &str,
    input_format: &str,
    delimiter: &str,
    header: &str,
    zebra: &str,
    theme: &str,
    accent: &str,
    font_size: &str,
    cell_padding: &str,
    title: &str,
    align: &str,
) -> Result<String, JsValue> {
    if input.trim().is_empty() {
        return Ok(String::new());
    }
    let opts = Options {
        input_format: if input_format.trim().is_empty() {
            "auto".into()
        } else {
            input_format.into()
        },
        delimiter: if delimiter.trim().is_empty() {
            ",".into()
        } else {
            delimiter.into()
        },
        header: parse_bool(header, true),
        zebra: parse_bool(zebra, true),
        theme: if theme.trim().is_empty() {
            "light".into()
        } else {
            theme.into()
        },
        accent: if accent.trim().is_empty() {
            "#2563eb".into()
        } else {
            accent.into()
        },
        font_size: font_size.trim().parse::<u32>().unwrap_or(14),
        cell_padding: cell_padding.trim().parse::<u32>().unwrap_or(10),
        title: title.into(),
        align: if align.trim().is_empty() {
            "left".into()
        } else {
            align.into()
        },
    };
    render(input, &opts).map_err(|e| JsValue::from_str(&e))
}
