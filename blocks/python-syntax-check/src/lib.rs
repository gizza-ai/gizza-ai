//! gizza-ai/python-syntax-check — chat skill block on the shared tool abstraction.
//! Parses pasted Python 3 source with the rustpython-parser grammar and reports
//! the first compile-time error (SyntaxError / IndentationError / TabError) with
//! its line, column and a caret-marked source echo. Source is parsed, never
//! executed. The chat schema is single-sourced from descriptor() (which also
//! drives the CLI); handle() delegates to run_skill. Pure → all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_python_syntax_check_core::run_with_options;
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    code: String,
    #[serde(default = "default_mode")]
    mode: String,
    #[serde(default = "default_format")]
    format: String,
    #[serde(default = "on")]
    show_context: bool,
    #[serde(default = "on")]
    python2_hints: bool,
    #[serde(default = "on")]
    stats: bool,
    #[serde(default = "default_filename")]
    filename: String,
}

fn default_mode() -> String {
    "module".into()
}
fn default_format() -> String {
    "text".into()
}
fn default_filename() -> String {
    "<input>".into()
}
fn on() -> bool {
    true
}

/// Single source for the chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("code")
                .required()
                .describe("Python source code to check. It is parsed with the Python 3 grammar and never executed, so runtime errors such as NameError are out of scope."),
        )
        .param(
            Param::enumv("mode", ["module", "expression", "interactive"])
                .default("module")
                .describe("Which of CPython's compile() modes to parse with: module for a whole file, expression for a single expression as eval() accepts, interactive for a REPL-style block."),
        )
        .param(
            Param::enumv("format", ["text", "json"])
                .default("text")
                .describe("Output format: a human-readable compiler-style report, or JSON diagnostics for scripts and CI."),
        )
        .param(
            Param::boolean("show_context")
                .default(true)
                .describe("Echo the offending source line with a ^ caret under the exact column, the way a Python traceback does. Default on."),
        )
        .param(
            Param::boolean("python2_hints")
                .default(true)
                .describe("Explain Python-2-only constructs (print statement, except E comma name, raise E comma msg, exec statement, <>, backtick repr, 0755 octal) in Python 3 terms. Default on."),
        )
        .param(
            Param::boolean("stats")
                .default(true)
                .describe("Include the input size summary: total lines, non-empty lines and characters. Default on."),
        )
        .param(
            Param::string("filename")
                .default("<input>")
                .describe("Label used in the report, so output reads like a real compiler line, for example app.py:12:5. Defaults to <input>."),
        )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/python-syntax-check",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Check Python 3 syntax and pinpoint the error line and column",
    skill(
        description = "Check pasted Python 3 source for compile-time syntax errors without running it. Reports the first SyntaxError, IndentationError or TabError with its line, column, message and a caret-marked echo of the offending line, plus optional hints that translate Python-2-only constructs into Python 3. Supports CPython's module, expression and interactive compile modes, text or JSON output, a filename label, and input line/character stats. Runtime errors, code execution and PEP 8 style linting are deliberately out of scope.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "python-syntax-check", |a: Args| {
            run_with_options(
                &a.code,
                &a.mode,
                &a.format,
                a.show_context,
                a.python2_hints,
                a.stats,
                &a.filename,
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
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived["required"], serde_json::json!(["code"]));
        assert_eq!(
            derived["properties"]["mode"]["enum"],
            serde_json::json!(["module", "expression", "interactive"])
        );
        assert_eq!(
            derived["properties"]["format"]["enum"],
            serde_json::json!(["text", "json"])
        );
        for key in ["show_context", "python2_hints", "stats"] {
            assert_eq!(derived["properties"][key]["type"], "boolean");
            assert_eq!(derived["properties"][key]["default"], true);
        }
        for key in [
            "code",
            "mode",
            "format",
            "show_context",
            "python2_hints",
            "stats",
            "filename",
        ] {
            assert!(
                derived["properties"][key]["description"]
                    .as_str()
                    .unwrap()
                    .len()
                    > 10,
                "{key} needs a real description"
            );
        }
    }
}
