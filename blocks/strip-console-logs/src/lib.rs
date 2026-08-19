//! gizza-ai/strip-console-logs — remove console.* debug statements from JavaScript
//! or TypeScript source. Chat schema single-sourced from descriptor() (which also
//! drives the CLI); handle() delegates to run_skill. Pure → all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_strip_console_logs_core::strip;
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    code: String,
    #[serde(default = "default_methods")]
    methods: String,
    #[serde(default)]
    keep: String,
    #[serde(default = "default_action")]
    action: String,
    #[serde(default)]
    remove_debugger: bool,
    #[serde(default = "default_output")]
    output: String,
}

fn default_methods() -> String {
    "log,debug,info,warn".to_string()
}

fn default_action() -> String {
    "remove".to_string()
}

fn default_output() -> String {
    "code".to_string()
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("code")
                .required()
                .describe("The JavaScript, TypeScript, JSX or TSX source to clean up. Max 500000 characters."),
        )
        .param(
            Param::string("methods")
                .default("log,debug,info,warn")
                .describe("Comma-separated console methods to strip, e.g. \"log,debug\". Use \"all\" for every console.* call. Names are case-insensitive and an optional \"console.\" prefix is allowed. Default \"log,debug,info,warn\"."),
        )
        .param(
            Param::string("keep")
                .default("")
                .describe("Comma-separated console methods to never strip, e.g. \"error,warn\". Wins over methods, so methods=all with keep=error is an exclude list. Default empty."),
        )
        .param(
            Param::enumv("action", ["remove", "comment", "blank"])
                .default("remove")
                .describe("What to do with each matched statement: \"remove\" deletes it (dropping the line when it held nothing else), \"comment\" comments it out in place, \"blank\" replaces it with blank lines so every later line keeps its number. Default \"remove\"."),
        )
        .param(
            Param::boolean("remove_debugger")
                .default(false)
                .describe("Also drop standalone `debugger;` statements. Default false."),
        )
        .param(
            Param::enumv("output", ["code", "report"])
                .default("code")
                .describe("\"code\" returns the rewritten source; \"report\" is a dry run that leaves the source alone and lists the line number of every statement it would remove, a per-method tally, and the calls it deliberately kept. Default \"code\"."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct StripConsoleLogs;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/strip-console-logs",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Remove console.log and other console.* statements from JavaScript or TypeScript",
    skill(
        description = "Strip console.* debug statements out of JavaScript or TypeScript source before shipping it. The scanner is token-aware: a console.log written inside a string, template literal, regular expression or comment is never touched, and multi-line calls with nested parentheses are matched as a whole. Pick the methods to strip (default log, debug, info, warn), use methods=all for every console call, and keep=error,warn as an exclude list. action=remove deletes the statement, action=comment comments it out, action=blank keeps line numbers stable. remove_debugger also drops `debugger;`. output=report is a dry run listing line numbers and per-method counts instead of rewriting. Calls used as a value (const a = console.log(x), x && console.log(y), arrow bodies, chained calls) are deliberately left in place because deleting them would change behaviour; an un-braced if/for/while/else/do body becomes an empty statement so the control flow still parses. Runs locally.",
        parameters = schema_json()
    ),
)]
impl StripConsoleLogs {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "strip-console-logs", |a: Args| {
            strip(
                &a.code,
                &a.methods,
                &a.keep,
                &a.action,
                a.remove_debugger,
                &a.output,
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
                    "code": {
                        "type": "string",
                        "description": "The JavaScript, TypeScript, JSX or TSX source to clean up. Max 500000 characters."
                    },
                    "methods": {
                        "type": "string",
                        "default": "log,debug,info,warn",
                        "description": "Comma-separated console methods to strip, e.g. \"log,debug\". Use \"all\" for every console.* call. Names are case-insensitive and an optional \"console.\" prefix is allowed. Default \"log,debug,info,warn\"."
                    },
                    "keep": {
                        "type": "string",
                        "default": "",
                        "description": "Comma-separated console methods to never strip, e.g. \"error,warn\". Wins over methods, so methods=all with keep=error is an exclude list. Default empty."
                    },
                    "action": {
                        "type": "string",
                        "enum": ["remove", "comment", "blank"],
                        "default": "remove",
                        "description": "What to do with each matched statement: \"remove\" deletes it (dropping the line when it held nothing else), \"comment\" comments it out in place, \"blank\" replaces it with blank lines so every later line keeps its number. Default \"remove\"."
                    },
                    "remove_debugger": {
                        "type": "boolean",
                        "default": false,
                        "description": "Also drop standalone `debugger;` statements. Default false."
                    },
                    "output": {
                        "type": "string",
                        "enum": ["code", "report"],
                        "default": "code",
                        "description": "\"code\" returns the rewritten source; \"report\" is a dry run that leaves the source alone and lists the line number of every statement it would remove, a per-method tally, and the calls it deliberately kept. Default \"code\"."
                    }
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
