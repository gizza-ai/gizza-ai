//! Markdown twin of each tool page (`index.md`), single-sourced from the same
//! `ToolMeta` (meta.toml) + content.md that drive the HTML page, so web-browsing
//! LLMs can read a tool without parsing HTML.

use crate::control::{fmt_num, Control, ParamSchema};
use crate::meta::{Input, ToolMeta};

/// Public site origin — matches the literal used in `template.rs`.
const SITE: &str = "https://gizza.ai";

/// Render a tool's `index.md`: a generated header (description, run-it, inputs,
/// output) followed by the tool's prose `content.md`, verbatim.
pub fn tool_markdown(meta: &ToolMeta, content_md: &str, schema: &ParamSchema) -> String {
    let mut s = String::new();
    s.push_str(&format!("# {}\n\n", meta.h1));
    s.push_str(&format!("{}\n\n", meta.description));

    s.push_str("## Run it\n\n");
    s.push_str(&format!("- **CLI:** `{}`\n", cli_example(meta)));
    s.push_str(&format!("- **Web:** {}/tools/{}/\n\n", SITE, meta.slug));

    s.push_str("## Inputs\n\n");
    let manual: Vec<&Input> = meta
        .inputs
        .iter()
        .filter(|i| i.source == "field" || i.source == "file")
        .collect();
    if manual.is_empty() {
        s.push_str("- _no manual inputs — runs automatically_\n\n");
    } else {
        for i in &manual {
            let label = if i.label.is_empty() { i.name.as_str() } else { i.label.as_str() };
            let mut note = i.source.clone();
            if !i.accept.is_empty() {
                note.push_str(&format!("; accept: {}", i.accept));
            }
            s.push_str(&format!("- `{}` — {} _({})_\n", i.name, label, note));
        }
        s.push('\n');
    }

    s.push_str("## Output\n\n");
    s.push_str(&format!("- {} ({})\n\n", meta.output_label, meta.format));

    // Query parameters: every page tool can be opened pre-filled and auto-run by
    // URL. Param names == input names; a file input is driven by ?url=. This is
    // the surface an LLM reads to drive the tool by link.
    let qp: Vec<&Input> = meta.inputs.iter().filter(|i| i.source == "field").collect();
    let has_file = meta.inputs.iter().any(|i| i.source == "file");
    if !qp.is_empty() || has_file {
        s.push_str("## Query parameters\n\n");
        s.push_str("Open the tool pre-filled and auto-run via URL:\n\n");
        for i in &qp {
            let label = if i.label.is_empty() { i.name.as_str() } else { i.label.as_str() };
            s.push_str(&format!("- `{}` — {}\n", i.name, label));
        }
        if has_file {
            s.push_str("- `url` — fetch the input file from a public URL (CORS-permitting)\n");
        }
        s.push_str(&format!("\nExample: `{}`\n\n", example_deeplink(meta, schema)));
    }

    s.push_str("---\n\n");
    s.push_str(content_md.trim_end());
    s.push('\n');
    s
}

/// The example CLI invocation: field tools use the first field input's
/// placeholder as the example arg; file tools take a path; auto-only tools
/// (e.g. clock) take no args.
fn cli_example(meta: &ToolMeta) -> String {
    if let Some(field) = meta.inputs.iter().find(|i| i.source == "field") {
        let arg = if field.placeholder.is_empty() { "..." } else { field.placeholder.as_str() };
        format!("gizza tool {} \"{}\"", meta.slug, arg)
    } else if meta.inputs.iter().any(|i| i.source == "file") {
        format!("gizza tool {} <path>", meta.slug)
    } else {
        format!("gizza tool {}", meta.slug)
    }
}

/// Example deep-link URL for the "Query parameters" docs (markdown + page).
/// Each field uses a real sample value derived from its schema control — a
/// select's default/first option, `true` for a checkbox, a number's
/// placeholder/default/min, else the field placeholder — so the example is
/// copy-pasteable rather than `=value`. A file input becomes `?url=…`.
/// `pub(crate)` so `template.rs` renders the same example.
pub(crate) fn example_deeplink(meta: &ToolMeta, schema: &ParamSchema) -> String {
    let mut pairs: Vec<String> = Vec::new();
    for i in &meta.inputs {
        if i.source == "file" {
            pairs.push("url=https://example.com/input".to_string());
        } else if i.source == "field" {
            let sample = sample_value(&schema.control_for_input(i), &i.placeholder);
            pairs.push(format!("{}={}", i.name, urlencode(&sample)));
        }
    }
    format!("{}/tools/{}/?{}", SITE, meta.slug, pairs.join("&"))
}

/// A realistic sample value for a field in the deep-link example.
fn sample_value(control: &Control, placeholder: &str) -> String {
    let ph = (!placeholder.is_empty()).then(|| placeholder.to_string());
    match control {
        Control::Select { options, default } => default
            .clone()
            .or_else(|| options.first().cloned())
            .unwrap_or_else(|| "value".to_string()),
        Control::Checkbox { .. } => "true".to_string(),
        Control::Number { default, min, .. } => ph
            .or_else(|| default.map(fmt_num))
            .or_else(|| min.map(fmt_num))
            .unwrap_or_else(|| "1".to_string()),
        Control::Picker { input_type } => ph.unwrap_or_else(|| {
            match input_type.as_str() {
                "time" => "09:30",
                "datetime-local" => "2000-01-31T09:30",
                _ => "2000-01-31", // "date"
            }
            .to_string()
        }),
        Control::Datalist { options } | Control::TagList { options } => ph
            .or_else(|| options.first().cloned())
            .unwrap_or_else(|| "value".to_string()),
        Control::Text | Control::Textarea => ph.unwrap_or_else(|| "value".to_string()),
    }
}

/// Minimal percent-encoding for example URLs (keeps RFC 3986 unreserved chars).
fn urlencode(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn field_tool() -> ToolMeta {
        ToolMeta::from_toml(
            r#"
slug          = "calculator"
title         = "Free Online Calculator — gizza.ai"
description   = "Evaluate expressions instantly."
tags          = ["math"]
h1            = "Free Online Calculator"
hero_subtitle = "Type a math expression."
wasm          = "gizza_ai_calculator_web"
export        = "evaluate"
live          = false
output_label  = "Result"
format        = "number"

[[input]]
name        = "expr"
label       = "Expression"
placeholder = "2 + 2 * 3"
source      = "field"
"#,
        )
        .unwrap()
    }

    fn file_tool() -> ToolMeta {
        ToolMeta::from_toml(
            r#"
slug          = "image-grayscale"
title         = "Grayscale an Image — gizza.ai"
description   = "Convert an image to grayscale in your browser."
h1            = "Grayscale an Image"
hero_subtitle = "Upload an image."
wasm          = "gizza_ai_image_grayscale_web"
export        = "build_argv"
output_label  = "Image"
format        = "image"
runtime       = "ffmpeg"

[[input]]
name   = "file"
label  = "Image"
source = "file"
accept = "image/*"
"#,
        )
        .unwrap()
    }

    fn live_tool() -> ToolMeta {
        ToolMeta::from_toml(
            r#"
slug          = "clock"
title         = "Clock — gizza.ai"
description   = "The current time, live."
h1            = "Clock"
hero_subtitle = "Live time."
wasm          = "gizza_ai_clock_web"
export        = "now"
live          = true
output_label  = "Time"
format        = "text"

[[input]]
name   = "now"
label  = "Now"
source = "clock"
"#,
        )
        .unwrap()
    }

    #[test]
    fn field_tool_has_header_runit_inputs_output_prose() {
        let md = tool_markdown(&field_tool(), "Some **prose** about the calculator.", &crate::control::ParamSchema::empty());
        assert!(md.contains("# Free Online Calculator"));
        assert!(md.contains("`gizza tool calculator \"2 + 2 * 3\"`"), "CLI example");
        assert!(md.contains("https://gizza.ai/tools/calculator/"), "web URL");
        assert!(md.contains("`expr` — Expression _(field)_"), "input listed");
        assert!(md.contains("Result (number)"), "output listed");
        assert!(md.contains("Some **prose** about the calculator."), "prose appended");
    }

    #[test]
    fn file_tool_uses_path_example_and_accept() {
        let md = tool_markdown(&file_tool(), "prose", &crate::control::ParamSchema::empty());
        assert!(md.contains("`gizza tool image-grayscale <path>`"), "path CLI example");
        assert!(md.contains("`file` — Image _(file; accept: image/*)_"), "accept shown");
    }

    #[test]
    fn live_tool_takes_no_manual_arguments() {
        let md = tool_markdown(&live_tool(), "prose", &crate::control::ParamSchema::empty());
        assert!(md.contains("`gizza tool clock`"), "no-arg CLI example");
        assert!(md.contains("_no manual inputs — runs automatically_"), "no manual inputs note");
    }

    #[test]
    fn field_tool_documents_query_parameters_with_example() {
        let md = tool_markdown(&field_tool(), "prose", &crate::control::ParamSchema::empty());
        assert!(md.contains("## Query parameters"), "query-params section present");
        assert!(md.contains("- `expr`"), "field param listed");
        assert!(
            md.contains("https://gizza.ai/tools/calculator/?expr="),
            "example deep-link present"
        );
    }

    #[test]
    fn file_tool_documents_url_query_parameter() {
        let md = tool_markdown(&file_tool(), "prose", &crate::control::ParamSchema::empty());
        assert!(md.contains("## Query parameters"));
        assert!(md.contains("- `url`"), "media tools document ?url=");
    }

    #[test]
    fn live_tool_has_no_query_parameters_section() {
        // clock has only a "clock" input — nothing to deep-link.
        let md = tool_markdown(&live_tool(), "prose", &crate::control::ParamSchema::empty());
        assert!(!md.contains("## Query parameters"), "no query-params for auto-only tools");
    }
}
