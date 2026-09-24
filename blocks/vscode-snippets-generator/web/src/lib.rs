//! Browser-facing wasm-bindgen wrapper for /tools/vscode-snippets-generator/.
use wasm_bindgen::prelude::*;

fn or_default(s: &str, fallback: &str) -> String {
    if s.trim().is_empty() { fallback.to_string() } else { s.trim().to_string() }
}

fn parse_f64(name: &str, s: &str, fallback: f64) -> Result<f64, JsValue> {
    let t = s.trim();
    if t.is_empty() { Ok(fallback) } else { t.parse::<f64>().map_err(|_| JsValue::from_str(&format!("{name} must be a number, got `{t}`"))) }
}

fn parse_bool(s: &str, fallback: bool) -> bool {
    match s.trim().to_ascii_lowercase().as_str() {
        "" => fallback,
        "true" | "1" | "yes" | "on" => true,
        "false" | "0" | "no" | "off" => false,
        _ => fallback,
    }
}

#[wasm_bindgen]
#[allow(clippy::too_many_arguments)]
pub fn run(
    template: &str,
    name: &str,
    prefix: &str,
    description: &str,
    scope: &str,
    output: &str,
    dollars: &str,
    indent: &str,
    tab_size: &str,
    final_tabstop: &str,
    is_file_template: &str,
    json_indent: &str,
) -> Result<String, JsValue> {
    gizza_ai_vscode_snippets_generator_core::run(
        template,
        name,
        prefix,
        description,
        scope,
        &or_default(output, "snippets-file"),
        &or_default(dollars, "auto"),
        &or_default(indent, "keep"),
        parse_f64("tab_size", tab_size, 2.0)?,
        parse_bool(final_tabstop, false),
        parse_bool(is_file_template, false),
        parse_f64("json_indent", json_indent, 2.0)?,
    )
    .map_err(|e| JsValue::from_str(&e))
}
