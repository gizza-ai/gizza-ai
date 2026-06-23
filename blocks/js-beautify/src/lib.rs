//! gizza-ai/js-beautify — re-indent and format obfuscated or minified
//! JavaScript into readable source. Chat schema single-sourced from descriptor()
//! (which also drives the CLI); handle() delegates to run_skill. Pure → all
//! backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_js_beautify_core::{beautify_with, IndentChar};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    code: String,
    #[serde(default = "default_indent")]
    indent: u64,
    #[serde(default = "default_indent_char")]
    indent_char: String,
}

fn default_indent() -> u64 {
    2
}

fn default_indent_char() -> String {
    "space".to_string()
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("code")
                .required()
                .describe("The JavaScript source to re-indent and format (minified or obfuscated input is fine)."),
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
                .describe("Indent with spaces or a tab character per level. Default space."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct JsBeautify;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/js-beautify",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Re-indent and format minified or obfuscated JavaScript",
    skill(
        description = "Re-indent and pretty-print minified or obfuscated JavaScript into clean, readable source. Puts one statement per line and indents nested blocks, objects and arrays. indent is spaces per nesting level (1-8, default 2). Strings, template literals, regular expressions and comments are preserved verbatim, so the output is semantically identical to the input — only whitespace and line breaks change. Runs locally.",
        parameters = schema_json()
    ),
)]
impl JsBeautify {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "js-beautify", |a: Args| {
            let ch = if a.indent_char.eq_ignore_ascii_case("tab") {
                IndentChar::Tab
            } else {
                IndentChar::Space
            };
            beautify_with(&a.code, a.indent as usize, ch).map_err(SkillError::InvalidArgs)
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
                    "code":   { "type": "string", "description": "The JavaScript source to re-indent and format (minified or obfuscated input is fine)." },
                    "indent": { "type": "integer", "minimum": 1, "maximum": 8, "default": 2, "description": "Spaces of indentation per nesting level (1-8). Ignored when indent_char is 'tab'. Default 2." },
                    "indent_char": { "type": "string", "enum": ["space", "tab"], "default": "space", "description": "Indent with spaces or a tab character per level. Default space." }
                },
                "required": ["code"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
