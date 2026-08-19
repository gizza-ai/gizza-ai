//! gizza-ai/indent-block-text — add, remove, or normalize indentation on a block of text.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    text: String,
    #[serde(default = "default_mode")]
    mode: String,
    #[serde(default = "default_style")]
    style: String,
    #[serde(default = "default_count")]
    count: i64,
    #[serde(default)]
    prefix: String,
    #[serde(default = "default_lines")]
    lines: String,
    #[serde(default = "default_true")]
    skip_blank_lines: bool,
}

fn default_mode() -> String {
    "indent".to_string()
}
fn default_style() -> String {
    "spaces".to_string()
}
fn default_count() -> i64 {
    4
}
fn default_lines() -> String {
    "all".to_string()
}
fn default_true() -> bool {
    true
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("text").required().describe("The text block to re-indent. Line endings are preserved, including a final trailing newline. Use this for code snippets, Markdown quotes, email replies, YAML fragments, or any multi-line text where each line needs the same leading spaces, tabs, or prefix."))
        .param(Param::enumv("mode", ["indent", "outdent", "dedent"]).default("indent").describe("What to do with leading indentation. indent (default) adds the chosen unit to selected lines. outdent removes up to count copies of the chosen unit from selected lines. dedent removes the common leading whitespace shared by selected non-blank lines, like Python textwrap.dedent."))
        .param(Param::enumv("style", ["spaces", "tabs", "custom"]).default("spaces").describe("Indent unit to add or remove. spaces uses one space per count, tabs uses one tab per count, and custom repeats the exact prefix string. For example style=custom prefix='> ' creates Markdown/email quote lines."))
        .param(Param::integer("count").default(4).min(0.0).max(200.0).describe("How many indent units to add or remove, from 0 to 200. With style=spaces, count=4 means four spaces. With style=tabs, count=2 means two tabs. With style=custom, count repeats the prefix that many times. Ignored by mode=dedent."))
        .param(Param::string("prefix").default("").describe("Custom prefix used when style=custom, such as '> ', '# ', '// ', or '│ '. It may contain Unicode and spaces and is repeated count times. Maximum 100 characters."))
        .param(Param::enumv("lines", ["all", "first-line", "hanging", "paragraph-starts"]).default("all").describe("Which lines to touch. all changes every selected line. first-line changes only the first line. hanging changes every line except the first. paragraph-starts changes the first line and each line after a blank line."))
        .param(Param::boolean("skip_blank_lines").default(true).describe("Leave blank or whitespace-only lines unchanged instead of adding indentation to them. Default true avoids trailing whitespace on empty lines. Set false when you need blank lines to carry the same prefix."))
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

fn run_args(a: Args) -> Result<String, String> {
    gizza_ai_indent_block_text_core::run_with_options(
        &a.text,
        &a.mode,
        &a.style,
        a.count,
        &a.prefix,
        &a.lines,
        a.skip_blank_lines,
    )
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/indent-block-text",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Add spaces, tabs, or a custom prefix to selected lines, or outdent/dedent text.",
    skill(
        description = "Add a configurable number of spaces, tabs, or copies of a custom prefix to the start of every selected line in a text block. It can also outdent by removing the same unit, or dedent by removing common leading whitespace. Choose all lines, first line, hanging indent, or paragraph starts; optionally skip blank lines to avoid trailing whitespace. Useful for code blocks, Markdown quotes, email replies, bibliography hanging indents, and batch prefixing.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "indent-block-text", |a: Args| {
            run_args(a).map_err(SkillError::InvalidArgs)
        }) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_exposes_all_options() {
        let schema: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        let props = schema["properties"].as_object().unwrap();
        for name in [
            "text",
            "mode",
            "style",
            "count",
            "prefix",
            "lines",
            "skip_blank_lines",
        ] {
            assert!(props.contains_key(name), "missing {name}");
            assert!(props[name]["description"].as_str().unwrap_or("").len() > 40);
        }
        assert_eq!(schema["required"], serde_json::json!(["text"]));
        assert_eq!(
            props["mode"]["enum"],
            serde_json::json!(["indent", "outdent", "dedent"])
        );
    }

    #[test]
    fn defaults_indent_four_spaces() {
        let out = run_args(Args {
            text: "a\nb".into(),
            mode: default_mode(),
            style: default_style(),
            count: default_count(),
            prefix: String::new(),
            lines: default_lines(),
            skip_blank_lines: true,
        })
        .unwrap();
        assert_eq!(out, "    a\n    b");
    }
}
