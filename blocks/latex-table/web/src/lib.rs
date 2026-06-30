//! Browser-facing wasm-bindgen wrapper for /tools/latex-table/.
//! Field order MUST match meta.toml: data, delimiter, style, alignment, header,
//! escape, bold_header, caption, label.
use gizza_ai_latex_table_core::{to_latex, Options, Style};
use wasm_bindgen::prelude::*;

fn truthy(v: &str) -> bool {
    matches!(v.trim().to_ascii_lowercase().as_str(), "true" | "1" | "on" | "yes")
}

#[wasm_bindgen]
#[allow(clippy::too_many_arguments)]
pub fn run(
    data: &str,
    delimiter: &str,
    style: &str,
    alignment: &str,
    header: &str,
    escape: &str,
    bold_header: &str,
    caption: &str,
    label: &str,
) -> Result<String, JsValue> {
    let style = Style::parse(style).map_err(|e| JsValue::from_str(&e))?;
    let align = if alignment.trim().is_empty() { "l" } else { alignment };
    let opts = Options {
        style,
        has_header: truthy(header),
        alignment: align,
        escape: truthy(escape),
        bold_header: truthy(bold_header),
        caption,
        label,
    };
    to_latex(data, delimiter, &opts).map_err(|e| JsValue::from_str(&e))
}
