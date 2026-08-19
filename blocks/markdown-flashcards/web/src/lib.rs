//! Browser-facing wasm-bindgen wrapper for /tools/markdown-flashcards/.
//! tool.js passes every field value as a raw string, so parse them here and funnel
//! through the same core the chat/CLI surfaces use.
use gizza_ai_markdown_flashcards_core::{
    generate, FieldFormat, FieldSep, Mode, Options, OutputKind,
};
use wasm_bindgen::prelude::*;

/// Checkboxes arrive as "true"/"false"; an empty value means the field was absent.
fn truthy(v: &str, default: bool) -> bool {
    match v.trim().to_ascii_lowercase().as_str() {
        "" => default,
        "true" | "1" | "on" | "yes" => true,
        _ => false,
    }
}

fn parse_level(v: &str) -> Result<u8, String> {
    let t = v.trim();
    if t.is_empty() {
        return Ok(0);
    }
    let n: f64 = t
        .parse()
        .map_err(|_| format!("heading level must be a number 0-6 (0 = auto-detect), got '{t}'"))?;
    if n.fract() != 0.0 || !(0.0..=6.0).contains(&n) {
        return Err(format!(
            "heading level must be a whole number 0-6 (0 = auto-detect), got '{t}'"
        ));
    }
    Ok(n as u8)
}

#[allow(clippy::too_many_arguments)]
#[wasm_bindgen]
pub fn run(
    markdown: &str,
    mode: &str,
    separator: &str,
    heading_level: &str,
    field_separator: &str,
    field_format: &str,
    notetype: &str,
    deck: &str,
    tags: &str,
    tags_from_headings: &str,
    include_headers: &str,
    dedupe: &str,
    output: &str,
) -> Result<String, JsValue> {
    let err = |e: String| JsValue::from_str(&e);
    let opts = Options {
        mode: Mode::parse(mode).map_err(err)?,
        separator: if separator.trim().is_empty() {
            "auto".to_string()
        } else {
            separator.to_string()
        },
        heading_level: parse_level(heading_level).map_err(err)?,
        field_separator: FieldSep::parse(field_separator).map_err(err)?,
        field_format: FieldFormat::parse(field_format).map_err(err)?,
        notetype: notetype.to_string(),
        deck: deck.to_string(),
        tags: tags.to_string(),
        tags_from_headings: truthy(tags_from_headings, false),
        include_headers: truthy(include_headers, true),
        dedupe: truthy(dedupe, true),
        output: OutputKind::parse(output).map_err(err)?,
    };
    generate(markdown, &opts).map_err(err)
}
