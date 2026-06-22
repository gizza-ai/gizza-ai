//! gizza-ai/html-formatter — pretty-print HTML with consistent indentation. Chat
//! schema single-sourced from descriptor(); handler delegates to run_skill.
//! Pure → all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_html_formatter_core::format;
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    html: String,
    #[serde(default = "default_indent")]
    indent: u64,
}

fn default_indent() -> u64 {
    2
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("html").required().describe("The HTML to pretty-print."))
        .param(
            Param::integer("indent")
                .min(0.0)
                .max(8.0)
                .default(2)
                .describe("Spaces of indentation per nesting level (0-8, default 2)."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct HtmlFormatter;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/html-formatter",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Pretty-print HTML with consistent indentation",
    skill(
        description = "Pretty-print (beautify) HTML with consistent indentation — one element per line, nested children indented. Handles void elements (br/img/etc.), self-closing tags, comments and doctype, and quoted attributes, and preserves the verbatim contents of pre/textarea/script/style. indent sets spaces per level (0-8, default 2). Runs locally.",
        parameters = schema_json()
    ),
)]
impl HtmlFormatter {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "html-formatter", |a: Args| {
            format(&a.html, a.indent as usize).map_err(SkillError::InvalidArgs)
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
                    "html":   { "type": "string", "description": "The HTML to pretty-print." },
                    "indent": { "type": "integer", "minimum": 0, "maximum": 8, "default": 2, "description": "Spaces of indentation per nesting level (0-8, default 2)." }
                },
                "required": ["html"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
