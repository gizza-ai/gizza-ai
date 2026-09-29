//! Browser-facing wasm-bindgen wrapper for /tools/xml-structure-analyzer/.
use gizza_ai_xml_structure_analyzer_core::{analyze, Format, Options};
use wasm_bindgen::prelude::*;

fn default_text(s: &str, fallback: &str) -> String {
    if s.trim().is_empty() {
        fallback.to_string()
    } else {
        s.trim().to_ascii_lowercase()
    }
}

fn flag(s: &str, fallback: bool) -> bool {
    match s.trim().to_ascii_lowercase().as_str() {
        "" => fallback,
        "true" | "1" | "on" | "yes" => true,
        _ => false,
    }
}

fn parse_usize(s: &str, fallback: usize, name: &str) -> Result<usize, JsValue> {
    let t = s.trim().replace([',', '_'], "");
    if t.is_empty() {
        return Ok(fallback);
    }
    t.parse::<usize>().map_err(|_| {
        JsValue::from_str(&format!(
            "{name} must be a whole number, got `{}`",
            s.trim()
        ))
    })
}

fn format_from(s: &str) -> Result<Format, JsValue> {
    match default_text(s, "text").as_str() {
        "text" | "txt" | "plain" => Ok(Format::Text),
        "json" => Ok(Format::Json),
        "csv" => Ok(Format::Csv),
        other => Err(JsValue::from_str(&format!("unknown format '{other}'"))),
    }
}

#[wasm_bindgen]
pub fn run(
    xml: &str,
    format: &str,
    tree_depth: &str,
    top_tags: &str,
    show_attributes: &str,
) -> Result<String, JsValue> {
    let opts = Options {
        format: format_from(format)?,
        tree_depth: parse_usize(tree_depth, 0, "tree_depth")?,
        top_tags: parse_usize(top_tags, 50, "top_tags")?,
        show_attributes: flag(show_attributes, true),
    };
    analyze(xml, &opts).map_err(|e| JsValue::from_str(&e))
}
