//! Markdown twin of each tool page (`index.md`), single-sourced from the same
//! `ToolMeta` (meta.toml) + content.md that drive the HTML page, so web-browsing
//! LLMs can read a tool without parsing HTML.

use crate::meta::{Input, ToolMeta};

/// Public site origin — matches the literal used in `seo.rs` + `template.rs`.
const SITE: &str = "https://gizza.ai";

/// Render a tool's `index.md`: a generated header (description, run-it, inputs,
/// output) followed by the tool's prose `content.md`, verbatim.
pub fn tool_markdown(meta: &ToolMeta, content_md: &str) -> String {
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
        let md = tool_markdown(&field_tool(), "Some **prose** about the calculator.");
        assert!(md.contains("# Free Online Calculator"));
        assert!(md.contains("`gizza tool calculator \"2 + 2 * 3\"`"), "CLI example");
        assert!(md.contains("https://gizza.ai/tools/calculator/"), "web URL");
        assert!(md.contains("`expr` — Expression _(field)_"), "input listed");
        assert!(md.contains("Result (number)"), "output listed");
        assert!(md.contains("Some **prose** about the calculator."), "prose appended");
    }

    #[test]
    fn file_tool_uses_path_example_and_accept() {
        let md = tool_markdown(&file_tool(), "prose");
        assert!(md.contains("`gizza tool image-grayscale <path>`"), "path CLI example");
        assert!(md.contains("`file` — Image _(file; accept: image/*)_"), "accept shown");
    }

    #[test]
    fn live_tool_takes_no_manual_arguments() {
        let md = tool_markdown(&live_tool(), "prose");
        assert!(md.contains("`gizza tool clock`"), "no-arg CLI example");
        assert!(md.contains("_no manual inputs — runs automatically_"), "no manual inputs note");
    }
}
