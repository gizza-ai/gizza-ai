//! gizza-ai/text-colorizer — chat skill block on the shared tool abstraction.
//! Applies user-defined `color: regex` rules to text and exports ANSI or HTML.
//! The chat schema is single-sourced from descriptor() (which also drives the
//! CLI); handle() delegates to block_utils::run_skill. Pure → all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_text_colorizer_core::colorize;
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    text: String,
    rules: String,
    #[serde(default)]
    output: String,
    #[serde(default)]
    theme: String,
    #[serde(default)]
    ignore_case: bool,
    #[serde(default)]
    whole_line: bool,
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("text")
                .required()
                .describe("The log or command output to colorize."),
        )
        .param(
            Param::string("rules")
                .required()
                .describe(
                    "Highlight rules, one per line as 'color-spec: regex'. The color spec (before the first colon) is space-separated tokens: optional attributes (bold, dim, italic, underline, blink, reverse, strike), a foreground color (name or #rgb/#rrggbb), and an optional 'on <color>' background. Colors: black, red, green, yellow, blue, magenta, cyan, white, their bright* variants, and gray. The rest of the line is a Rust regex. Example: 'bold red: \\bERROR\\b'.",
                ),
        )
        .param(
            Param::enumv("output", ["ansi", "html"])
                .default("ansi")
                .describe("Output format: 'ansi' terminal escape codes (default) or a self-contained styled HTML <pre>."),
        )
        .param(
            Param::enumv("theme", ["dark", "light"])
                .default("dark")
                .describe("HTML background/foreground theme (ignored for ANSI output)."),
        )
        .param(
            Param::boolean("ignore_case")
                .default(false)
                .describe("Match every rule case-insensitively."),
        )
        .param(
            Param::boolean("whole_line")
                .default(false)
                .describe("Color the entire line matched by the first matching rule, instead of only the matched substrings."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct TextColorizer;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/text-colorizer",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Highlight text with user-defined regex color rules, as ANSI or HTML",
    skill(
        description = "Apply user-defined regex color rules to log or command output and export the result as ANSI terminal escapes or self-contained HTML. Provide the text plus rules, one per line as 'color-spec: regex' (e.g. 'bold red: \\bERROR\\b'). The color spec supports named colors (red, green, brightyellow, …), #rgb/#rrggbb hex, attributes (bold, italic, underline, …) and an 'on <color>' background. Toggle ignore_case for case-insensitive matching and whole_line to color the entire matched line. Earlier rules win on overlap. Runs locally.",
        parameters = schema_json()
    ),
)]
impl TextColorizer {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "text-colorizer", |a: Args| {
            colorize(
                &a.text,
                &a.rules,
                &a.output,
                &a.theme,
                a.ignore_case,
                a.whole_line,
            )
            .map_err(SkillError::InvalidArgs)
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
                    "text": { "type": "string", "description": "The log or command output to colorize." },
                    "rules": { "type": "string", "description": "Highlight rules, one per line as 'color-spec: regex'. The color spec (before the first colon) is space-separated tokens: optional attributes (bold, dim, italic, underline, blink, reverse, strike), a foreground color (name or #rgb/#rrggbb), and an optional 'on <color>' background. Colors: black, red, green, yellow, blue, magenta, cyan, white, their bright* variants, and gray. The rest of the line is a Rust regex. Example: 'bold red: \\bERROR\\b'." },
                    "output": { "type": "string", "enum": ["ansi", "html"], "default": "ansi", "description": "Output format: 'ansi' terminal escape codes (default) or a self-contained styled HTML <pre>." },
                    "theme": { "type": "string", "enum": ["dark", "light"], "default": "dark", "description": "HTML background/foreground theme (ignored for ANSI output)." },
                    "ignore_case": { "type": "boolean", "default": false, "description": "Match every rule case-insensitively." },
                    "whole_line": { "type": "boolean", "default": false, "description": "Color the entire line matched by the first matching rule, instead of only the matched substrings." }
                },
                "required": ["text", "rules"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
