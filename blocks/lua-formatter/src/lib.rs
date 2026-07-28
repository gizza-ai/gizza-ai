//! gizza-ai/lua-formatter — a Lua source re-indenter / beautifier on the shared
//! tool abstraction. The chat schema is single-sourced from descriptor() (which
//! also drives the CLI); handle() parses the params and delegates to the pure
//! core::format. Pure → all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_lua_formatter_core::{format, Indent, QuoteStyle};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    input: String,
    #[serde(default = "default_indent")]
    indent: u64,
    #[serde(default = "default_indent_char")]
    indent_char: String,
    #[serde(default = "default_quote_style")]
    quote_style: String,
}

fn default_indent() -> u64 {
    2
}
fn default_indent_char() -> String {
    "space".to_string()
}
fn default_quote_style() -> String {
    "preserve".to_string()
}

/// Resolve the requested indentation into a core [`Indent`]: a tab per level, or
/// `width` spaces per level.
fn resolve_indent(width: u64, indent_char: &str) -> Indent {
    if indent_char.eq_ignore_ascii_case("tab") {
        Indent::Tab
    } else {
        Indent::Spaces(width as usize)
    }
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("input")
                .required()
                .describe("The Lua source code to re-indent and beautify (any dialect: 5.1-5.4, LuaJIT, Luau). Minified or messy input is fine."),
        )
        .param(
            Param::integer("indent")
                .min(1.0)
                .max(8.0)
                .default(2)
                .describe("Spaces of indentation per nesting level (1-8). Ignored when indent_char is 'tab'. Default 2."),
        )
        .param(
            Param::enumv("indent_char", ["space", "tab"])
                .default("space")
                .describe("Indent with spaces or one tab character per level. Default space."),
        )
        .param(
            Param::enumv("quote_style", ["preserve", "double", "single"])
                .default("preserve")
                .describe(
                    "Normalize short-string quotes: preserve (default, keep each string's own quote), double (\"...\"), or single ('...'). Re-escapes as needed; long strings [[...]] are never touched.",
                ),
        )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/lua-formatter",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Re-indent and beautify Lua source code",
    skill(
        description = "Re-indent and beautify Lua source code. A forgiving tokenizer rebuilds the source line by line: leading indentation is recomputed from block structure (function/then/do/repeat and (/[/{ open a level; end/until and closers close one), commas get one space after and none before, runs of blank lines collapse, and — optionally — short-string quotes are normalized to a single style. It never wraps long lines, never renames or reorders anything, and never touches the interior of a long string or long comment, so it is safe on any Lua dialect (5.1-5.4, LuaJIT, Luau). indent = spaces per level (1-8, default 2); indent_char = space (default) or tab; quote_style = preserve (default), double, or single. Runs locally.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "lua-formatter", |a: Args| {
            let style = QuoteStyle::parse(&a.quote_style).map_err(SkillError::InvalidArgs)?;
            let indent = resolve_indent(a.indent, &a.indent_char);
            format(&a.input, indent, style).map_err(SkillError::InvalidArgs)
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
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "input":       { "type": "string", "description": "The Lua source code to re-indent and beautify (any dialect: 5.1-5.4, LuaJIT, Luau). Minified or messy input is fine." },
                    "indent":      { "type": "integer", "minimum": 1, "maximum": 8, "default": 2, "description": "Spaces of indentation per nesting level (1-8). Ignored when indent_char is 'tab'. Default 2." },
                    "indent_char": { "type": "string", "enum": ["space", "tab"], "default": "space", "description": "Indent with spaces or one tab character per level. Default space." },
                    "quote_style": { "type": "string", "enum": ["preserve", "double", "single"], "default": "preserve", "description": "Normalize short-string quotes: preserve (default, keep each string's own quote), double (\"...\"), or single ('...'). Re-escapes as needed; long strings [[...]] are never touched." }
                },
                "required": ["input"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
