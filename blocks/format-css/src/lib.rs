//! gizza-ai/format-css — pretty-print CSS / SCSS / LESS with declaration
//! ordering, hex normalization and per-selector line splitting. Chat schema
//! single-sourced from descriptor() (which also drives the CLI); handle()
//! delegates to run_skill. Pure → all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_format_css_core::{format, parse_sort, Indent, Options};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    input: String,
    #[serde(default = "default_indent")]
    indent: u64,
    #[serde(default = "default_indent_char")]
    indent_char: String,
    #[serde(default = "default_sort")]
    sort: String,
    #[serde(default = "default_selectors_per_line")]
    selectors_per_line: bool,
    #[serde(default)]
    uppercase_hex: bool,
}

fn default_indent() -> u64 {
    2
}
fn default_indent_char() -> String {
    "space".to_string()
}
fn default_sort() -> String {
    "none".to_string()
}
fn default_selectors_per_line() -> bool {
    true
}

/// Resolve [`Args`] into core [`Options`], validating the enum params.
fn resolve(a: &Args) -> Result<Options, String> {
    let indent = if a.indent_char.eq_ignore_ascii_case("tab") {
        Indent::Tab
    } else {
        Indent::Spaces(a.indent as usize)
    };
    let sort = parse_sort(&a.sort).ok_or_else(|| format!("unknown sort '{}'", a.sort))?;
    Ok(Options {
        indent,
        sort,
        selectors_per_line: a.selectors_per_line,
        uppercase_hex: a.uppercase_hex,
    })
}

fn beautify(a: &Args) -> Result<String, String> {
    format(&a.input, resolve(a)?)
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("input")
                .required()
                .describe("The CSS, SCSS or LESS source to pretty-print (minified or messy input is fine)."),
        )
        .param(
            Param::integer("indent")
                .min(0.0)
                .max(8.0)
                .default(2)
                .describe("Spaces of indentation per nesting level (0-8). Ignored when indent_char is 'tab'. Default 2."),
        )
        .param(
            Param::enumv("indent_char", ["space", "tab"])
                .default("space")
                .describe("Indent with spaces or a tab character per level. Default space."),
        )
        .param(
            Param::enumv("sort", ["none", "alphabetical", "grouped"])
                .default("none")
                .describe("Order declarations within each rule: 'none' keeps source order, 'alphabetical' sorts A-Z by property, 'grouped' uses a concentric (positioning → box model → color → typography) order. Default none."),
        )
        .param(
            Param::boolean("selectors_per_line")
                .default(true)
                .describe("Put each comma-separated selector on its own line (h1, h2 → h1,\\nh2). Default true."),
        )
        .param(
            Param::boolean("uppercase_hex")
                .default(false)
                .describe("Uppercase the hex digits of #rgb/#rrggbb color values (#abcdef → #ABCDEF). Default false."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/format-css",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Pretty-print CSS, SCSS or LESS with optional sorting",
    skill(
        description = "Pretty-print (beautify) CSS, SCSS or LESS: one declaration per line, `prop: value` spacing normalized, nested rules (SCSS/LESS `&`, `@media`/`@mixin`) indented, comments preserved. indent sets spaces per level (0-8, default 2) or set indent_char to 'tab'. sort orders declarations within each rule ('none'/'alphabetical'/'grouped', default none). selectors_per_line (default true) splits comma selectors onto their own lines. uppercase_hex (default false) uppercases hex colors. Values are never rewritten (lossless); minification is a separate tool. Runs locally.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "format-css", |a: Args| {
            beautify(&a).map_err(SkillError::InvalidArgs)
        }) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(input: &str) -> Args {
        Args {
            input: input.to_string(),
            indent: 2,
            indent_char: "space".to_string(),
            sort: "none".to_string(),
            selectors_per_line: true,
            uppercase_hex: false,
        }
    }

    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "input":              { "type": "string", "description": "The CSS, SCSS or LESS source to pretty-print (minified or messy input is fine)." },
                    "indent":             { "type": "integer", "minimum": 0, "maximum": 8, "default": 2, "description": "Spaces of indentation per nesting level (0-8). Ignored when indent_char is 'tab'. Default 2." },
                    "indent_char":        { "type": "string", "enum": ["space", "tab"], "default": "space", "description": "Indent with spaces or a tab character per level. Default space." },
                    "sort":               { "type": "string", "enum": ["none", "alphabetical", "grouped"], "default": "none", "description": "Order declarations within each rule: 'none' keeps source order, 'alphabetical' sorts A-Z by property, 'grouped' uses a concentric (positioning → box model → color → typography) order. Default none." },
                    "selectors_per_line": { "type": "boolean", "default": true, "description": "Put each comma-separated selector on its own line (h1, h2 → h1,\\nh2). Default true." },
                    "uppercase_hex":      { "type": "boolean", "default": false, "description": "Uppercase the hex digits of #rgb/#rrggbb color values (#abcdef → #ABCDEF). Default false." }
                },
                "required": ["input"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }

    #[test]
    fn beautify_default() {
        assert_eq!(beautify(&args("a{color:red}")).unwrap(), "a {\n  color: red;\n}\n");
    }

    #[test]
    fn beautify_rejects_unknown_sort() {
        let mut a = args("a{}");
        a.sort = "bogus".to_string();
        assert!(beautify(&a).unwrap_err().contains("unknown sort"));
    }
}
