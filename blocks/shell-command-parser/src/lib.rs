//! gizza-ai/shell-command-parser — chat skill block on the shared tool abstraction.
//! The chat schema is single-sourced from descriptor() (which also drives the
//! CLI); handle() delegates to block_utils::run_skill.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    input: String,
    format: Option<String>,
    pretty: Option<bool>,
}

/// Single source for the chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("input")
                .required()
                .multiline()
                .placeholder("LC_ALL=C grep -rn 'FIXME' src/ 2>/dev/null | sort -u > matches.txt")
                .describe("Shell command line to parse. The command is analyzed only; it is never executed."),
        )
        .param(
            Param::enumv("format", ["json", "tree", "explain", "commands"])
                .default("json")
                .describe("Output format: structured JSON, an ASCII tree, a plain-English explanation, or a flat command table."),
        )
        .param(
            Param::boolean("pretty")
                .default(true)
                .describe("Pretty-print JSON output with indentation. Ignored by non-JSON formats."),
        )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn descriptor_schema_stays_in_sync() {
        let actual: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        let expected = serde_json::json!({
            "type": "object",
            "required": ["input"],
            "properties": {
                "input": {
                    "type": "string",
                    "description": "Shell command line to parse. The command is analyzed only; it is never executed."
                },
                "format": {
                    "type": "string",
                    "enum": ["json", "tree", "explain", "commands"],
                    "default": "json",
                    "description": "Output format: structured JSON, an ASCII tree, a plain-English explanation, or a flat command table."
                },
                "pretty": {
                    "type": "boolean",
                    "default": true,
                    "description": "Pretty-print JSON output with indentation. Ignored by non-JSON formats."
                }
            },
            "additionalProperties": false
        });
        assert_eq!(actual, expected);
    }
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/shell-command-parser",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Parse shell commands into JSON, trees, explanations, or command tables.",
    skill(
        description = "Parses a POSIX/bash-style shell command line into commands, pipes, redirects, assignments, quotes, globs, and expansions without executing anything.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "shell-command-parser", |a: Args| {
            let format = a.format.as_deref().unwrap_or("json");
            let pretty = a.pretty.unwrap_or(true);
            gizza_ai_shell_command_parser_core::run(&a.input, format, pretty)
                .map_err(SkillError::InvalidArgs)
        }) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}
