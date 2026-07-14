//! Markdown twin of each tool page (`index.md`), single-sourced from the same
//! `ToolMeta` (meta.toml) + content.md that drive the HTML page, so web-browsing
//! LLMs can read a tool without parsing HTML.

use crate::control::{fmt_num, Control, ParamSchema};
use crate::meta::{Input, ToolMeta};
use crate::site::SiteConfig;

/// Render a tool's `index.md`: a generated header (description, run-it, inputs,
/// output) followed by the tool's prose `content.md`, verbatim, and a
/// "Related tools" link list (same top-5 as the HTML page).
pub fn tool_markdown(
    cfg: &SiteConfig,
    meta: &ToolMeta,
    content_md: &str,
    schema: &ParamSchema,
    related: &[&ToolMeta],
) -> String {
    let mut s = String::new();
    s.push_str(&format!("# {}\n\n", meta.h1));
    s.push_str(&format!("{}\n\n", meta.description));

    s.push_str("## Run it\n\n");
    s.push_str(&format!("- **CLI:** `{}`\n", cli_example(meta, schema)));
    s.push_str(&format!("- **Web:** {}\n", cfg.url_or_rel(&format!("/tools/{}/", meta.slug))));
    s.push_str(&format!(
        "- **Agents:** machine-readable descriptor (parameters JSON Schema) at \
         {}\n\n",
        cfg.url_or_rel(&format!("/tools/{}/tool.json", meta.slug))
    ));

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
        s.push_str(&format!("\nExample: `{}`\n\n", example_deeplink(cfg, meta, schema)));
    }

    s.push_str("---\n\n");
    s.push_str(content_md.trim_end());
    s.push('\n');

    if !related.is_empty() {
        s.push_str("\n## Related tools\n\n");
        for r in related {
            s.push_str(&format!(
                "- [{}]({}): {}\n",
                r.h1,
                cfg.url_or_rel(&format!("/tools/{}/", r.slug)),
                r.description
            ));
        }
    }
    s
}

/// The example CLI invocation, copy-paste-runnable. File tools take
/// `url=…` plus a `key=value` sample for EVERY field input (derived from the
/// same schema samples as the deep-link example — a file tool's fields are
/// often required, so an example without them would just error). Pure field
/// tools use the first field's placeholder as the bare positional (the CLI
/// maps bare positionals against the schema's `required` list) — plus a
/// `key=value` sample for every OTHER required field, because an example
/// missing any required param errors verbatim ("missing required arg").
/// Auto-only tools (e.g. clock) take no args.
/// `pub(crate)` so `template.rs` renders the identical example on the page.
pub(crate) fn cli_example(meta: &ToolMeta, schema: &ParamSchema) -> String {
    if meta.inputs.iter().any(|i| i.source == "file") {
        let mut args = vec!["'url=https://example.com/input'".to_string()];
        for i in meta.inputs.iter().filter(|i| i.source == "field") {
            if let Some(sample) = sample_value(&schema.control_for_input(i), &i.placeholder) {
                args.push(format!("'{}={}'", i.name, sample));
            }
        }
        format!("gizza tool {} {}", meta.slug, args.join(" "))
    } else if let Some(field) = meta.inputs.iter().find(|i| i.source == "field") {
        let arg = if field.placeholder.is_empty() { "..." } else { field.placeholder.as_str() };
        let mut cmd = format!("gizza tool {} \"{}\"", meta.slug, arg);
        // The bare positional only covers the FIRST required scalar param.
        // Every other required field needs an explicit key=value or the
        // copy-pasted example fails. (With a stale/missing manifest the schema
        // knows nothing — keep the legacy single-positional form.)
        if schema.knows_params() && schema.is_required(&field.name) {
            for i in meta.inputs.iter().filter(|i| i.source == "field").skip(1) {
                if !schema.is_required(&i.name) {
                    continue;
                }
                if let Some(sample) = sample_value(&schema.control_for_input(i), &i.placeholder) {
                    cmd.push_str(&format!(" '{}={}'", i.name, sample));
                }
            }
        }
        cmd
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
pub(crate) fn example_deeplink(cfg: &SiteConfig, meta: &ToolMeta, schema: &ParamSchema) -> String {
    let mut pairs: Vec<String> = Vec::new();
    for i in &meta.inputs {
        if i.source == "file" {
            pairs.push("url=https://example.com/input".to_string());
        } else if i.source == "field" {
            if let Some(sample) = sample_value(&schema.control_for_input(i), &i.placeholder) {
                pairs.push(format!("{}={}", i.name, urlencode(&sample)));
            }
        }
    }
    cfg.url_or_rel(&format!("/tools/{}/?{}", meta.slug, pairs.join("&")))
}

/// A realistic sample value for a field in the deep-link example, or `None`
/// to omit the param from the example entirely.
fn sample_value(control: &Control, placeholder: &str) -> Option<String> {
    let ph = (!placeholder.is_empty()).then(|| placeholder.to_string());
    match control {
        Control::Select { options, default } => Some(
            default
                .clone()
                .or_else(|| options.first().cloned())
                .unwrap_or_else(|| "value".to_string()),
        ),
        Control::Checkbox { .. } => Some("true".to_string()),
        Control::Number { default, min, .. } => {
            // A non-numeric placeholder on a number field (e.g. trim-audio's
            // end hint "to end") means EMPTY is the meaningful value — omit
            // the param from the example instead of teaching a bogus value.
            let numeric_ph = ph.as_ref().filter(|p| p.parse::<f64>().is_ok()).cloned();
            if ph.is_some() && numeric_ph.is_none() {
                return None;
            }
            Some(
                numeric_ph
                    .or_else(|| default.map(fmt_num))
                    .or_else(|| min.map(fmt_num))
                    .unwrap_or_else(|| "1".to_string()),
            )
        }
        Control::Slider { default, min, .. } => {
            // Same numeric-sample rules as Number; a slider always has bounds,
            // so min is the last-resort sample.
            let numeric_ph = ph.as_ref().filter(|p| p.parse::<f64>().is_ok()).cloned();
            if ph.is_some() && numeric_ph.is_none() {
                return None;
            }
            Some(
                numeric_ph
                    .or_else(|| default.map(fmt_num))
                    .unwrap_or_else(|| fmt_num(*min)),
            )
        }
        Control::Color { default } => {
            // Same omit-rule as Number: a non-hex placeholder (e.g.
            // "transparent — or a hex like #0b1220") means EMPTY is the
            // meaningful value, so the param is left out of the example
            // rather than teaching a value that would be rejected.
            let hex_ph = ph.as_ref().and_then(|p| crate::control::expand_hex(p).map(|_| p.clone()));
            if ph.is_some() && hex_ph.is_none() {
                return None;
            }
            hex_ph.or_else(|| default.clone())
        }
        Control::Picker { input_type } => Some(ph.unwrap_or_else(|| {
            match input_type.as_str() {
                "time" => "09:30",
                "datetime-local" => "2000-01-31T09:30",
                _ => "2000-01-31", // "date"
            }
            .to_string()
        })),
        Control::Datalist { options } | Control::TagList { options } => Some(
            ph.or_else(|| options.first().cloned())
                .unwrap_or_else(|| "value".to_string()),
        ),
        Control::Text | Control::Textarea => Some(ph.unwrap_or_else(|| "value".to_string())),
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

    /// A branded config matching today's real `site/site-config.toml`
    /// (`base_url`/`brand_name` set) — most existing assertions here check
    /// the historical absolute `https://gizza.ai/...` links.
    fn branded() -> SiteConfig {
        SiteConfig { base_url: "https://gizza.ai".into(), brand_name: "gizza.ai".into(), ..Default::default() }
    }

    fn field_tool() -> ToolMeta {
        ToolMeta::from_toml(
            r#"
slug          = "calculator"
title         = "Free Online Calculator"
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

    #[test]
    fn number_sample_omits_non_numeric_placeholder() {
        // A non-numeric placeholder on a number control (trim-audio's end
        // "to end") signals that EMPTY is meaningful — the example must omit
        // the param rather than teach a bogus value.
        let num = Control::Number { min: Some(0.0), max: None, default: None, step_any: false };
        assert_eq!(sample_value(&num, "to end"), None);
        assert_eq!(sample_value(&num, "15"), Some("15".to_string()));
        assert_eq!(sample_value(&num, ""), Some("0".to_string())); // min fallback
    }

    fn file_tool() -> ToolMeta {
        ToolMeta::from_toml(
            r#"
slug          = "image-grayscale"
title         = "Grayscale an Image"
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
title         = "Clock"
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
        let md = tool_markdown(&branded(), &field_tool(), "Some **prose** about the calculator.", &crate::control::ParamSchema::empty(), &[]);
        assert!(md.contains("# Free Online Calculator"));
        assert!(md.contains("`gizza tool calculator \"2 + 2 * 3\"`"), "CLI example");
        assert!(md.contains("https://gizza.ai/tools/calculator/"), "web URL");
        assert!(md.contains("`expr` — Expression _(field)_"), "input listed");
        assert!(md.contains("Result (number)"), "output listed");
        assert!(md.contains("Some **prose** about the calculator."), "prose appended");
    }

    #[test]
    fn run_it_mentions_the_machine_readable_descriptor() {
        let md = tool_markdown(&branded(), &field_tool(), "prose", &crate::control::ParamSchema::empty(), &[]);
        assert!(
            md.contains(
                "- **Agents:** machine-readable descriptor (parameters JSON Schema) at \
                 https://gizza.ai/tools/calculator/tool.json"
            ),
            "Run it lists the tool.json descriptor"
        );
    }

    #[test]
    fn file_tool_uses_url_example_and_accept() {
        let md = tool_markdown(&branded(), &file_tool(), "prose", &crate::control::ParamSchema::empty(), &[]);
        assert!(
            md.contains("`gizza tool image-grayscale 'url=https://example.com/input'`"),
            "url= CLI example"
        );
        assert!(md.contains("`file` — Image _(file; accept: image/*)_"), "accept shown");
    }

    #[test]
    fn file_tool_cli_example_includes_field_samples() {
        // A file tool's field params are often REQUIRED — a copy-pasted
        // example without them would just error. Samples come from the same
        // schema-derived values as the deep-link example.
        let meta = ToolMeta::from_toml(
            r#"
slug          = "audio-pitch-shift"
title         = "t"
description   = "d"
h1            = "h"
hero_subtitle = "s"
wasm          = "w"
export        = "build_argv"
output_label  = "o"
format        = "audio"
runtime       = "ffmpeg"

[[input]]
name   = "audio"
source = "file"
accept = "audio/*"

[[input]]
name        = "semitones"
source      = "field"
placeholder = "3"

[[input]]
name   = "format"
source = "field"
"#,
        )
        .unwrap();
        let schema = ParamSchema::from_props_for_tests(serde_json::json!({
            "semitones": { "type": "number", "minimum": -24, "maximum": 24 },
            "format": { "type": "string", "enum": ["mp3", "wav"], "default": "mp3" }
        }));
        assert_eq!(
            cli_example(&meta, &schema),
            "gizza tool audio-pitch-shift 'url=https://example.com/input' 'semitones=3' 'format=mp3'"
        );
    }

    #[test]
    fn pure_tool_cli_example_covers_every_required_param() {
        // A pure tool with TWO required params: the bare positional only maps
        // to the first, so the example must pass the second as key=value —
        // otherwise the copy-pasted example errors ("missing required arg").
        // Optional params stay out of the example.
        let meta = ToolMeta::from_toml(
            r#"
slug          = "cartesian-product"
title         = "t"
description   = "d"
h1            = "h"
hero_subtitle = "s"
wasm          = "w"
export        = "run"
output_label  = "o"
format        = "text"

[[input]]
name        = "list1"
source      = "field"
placeholder = "red, blue, green"

[[input]]
name        = "list2"
source      = "field"
placeholder = "S, M, L"

[[input]]
name        = "list3"
source      = "field"
placeholder = "cotton, linen"
"#,
        )
        .unwrap();
        let schema = ParamSchema::from_props_for_tests(serde_json::json!({
            "list1": { "type": "string" },
            "list2": { "type": "string" },
            "list3": { "type": "string", "default": "" }
        }))
        .with_required_for_tests(&["list1", "list2"]);
        assert_eq!(
            cli_example(&meta, &schema),
            "gizza tool cartesian-product \"red, blue, green\" 'list2=S, M, L'"
        );
    }

    #[test]
    fn live_tool_takes_no_manual_arguments() {
        let md = tool_markdown(&branded(), &live_tool(), "prose", &crate::control::ParamSchema::empty(), &[]);
        assert!(md.contains("`gizza tool clock`"), "no-arg CLI example");
        assert!(md.contains("_no manual inputs — runs automatically_"), "no manual inputs note");
    }

    #[test]
    fn related_tools_appended_as_markdown_link_list() {
        let percentage = ToolMeta::from_toml(
            r#"
slug          = "percentage-calculator"
title         = "t"
description   = "Work out percentages instantly."
h1            = "Percentage Calculator"
hero_subtitle = "s"
wasm          = "w"
export        = "run"
output_label  = "o"
format        = "text"
"#,
        )
        .unwrap();
        let related = vec![&percentage];
        let md = tool_markdown(
            &branded(),
            &field_tool(),
            "prose",
            &crate::control::ParamSchema::empty(),
            &related,
        );
        assert!(md.contains("## Related tools"), "related section present");
        assert!(
            md.contains(
                "- [Percentage Calculator](https://gizza.ai/tools/percentage-calculator/): \
                 Work out percentages instantly."
            ),
            "related entry is an absolute markdown link with the description"
        );
        // the section comes AFTER the prose content
        assert!(md.find("prose").unwrap() < md.find("## Related tools").unwrap());

        // no related tools → no empty section
        let md = tool_markdown(&branded(), &field_tool(), "prose", &crate::control::ParamSchema::empty(), &[]);
        assert!(!md.contains("## Related tools"));
    }

    #[test]
    fn field_tool_documents_query_parameters_with_example() {
        let md = tool_markdown(&branded(), &field_tool(), "prose", &crate::control::ParamSchema::empty(), &[]);
        assert!(md.contains("## Query parameters"), "query-params section present");
        assert!(md.contains("- `expr`"), "field param listed");
        assert!(
            md.contains("https://gizza.ai/tools/calculator/?expr="),
            "example deep-link present"
        );
    }

    #[test]
    fn file_tool_documents_url_query_parameter() {
        let md = tool_markdown(&branded(), &file_tool(), "prose", &crate::control::ParamSchema::empty(), &[]);
        assert!(md.contains("## Query parameters"));
        assert!(md.contains("- `url`"), "media tools document ?url=");
    }

    #[test]
    fn live_tool_has_no_query_parameters_section() {
        // clock has only a "clock" input — nothing to deep-link.
        let md = tool_markdown(&branded(), &live_tool(), "prose", &crate::control::ParamSchema::empty(), &[]);
        assert!(!md.contains("## Query parameters"), "no query-params for auto-only tools");
    }

    #[test]
    fn default_config_uses_relative_urls_and_no_brand() {
        let md = tool_markdown(&SiteConfig::default(), &field_tool(), "prose", &crate::control::ParamSchema::empty(), &[]);
        assert!(md.contains("- **Web:** /tools/calculator/\n"), "relative web link");
        assert!(
            md.contains("- **Agents:** machine-readable descriptor (parameters JSON Schema) at /tools/calculator/tool.json"),
            "relative descriptor link"
        );
        assert!(md.contains("/tools/calculator/?expr="), "relative deep-link example");
        assert!(!md.contains("gizza.ai"), "no brand leaks into a generic render");
    }
}
