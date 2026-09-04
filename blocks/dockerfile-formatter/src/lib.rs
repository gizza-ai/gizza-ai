//! gizza-ai/dockerfile-formatter — normalize Dockerfile casing, indentation and continuations.
//! The chat schema is single-sourced from descriptor() (which also drives the
//! CLI); handle() delegates to block_utils::run_skill.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    input: String,
    #[serde(default = "default_case")]
    instruction_case: String,
    #[serde(default = "default_indent")]
    indent: usize,
    #[serde(default)]
    align_continuations: bool,
    #[serde(default = "default_max_blank_lines")]
    max_blank_lines: usize,
    #[serde(default = "default_true")]
    blank_line_between_stages: bool,
    #[serde(default = "default_true")]
    normalize_comments: bool,
}

fn default_case() -> String {
    "upper".to_string()
}
fn default_indent() -> usize {
    4
}
fn default_max_blank_lines() -> usize {
    1
}
fn default_true() -> bool {
    true
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("input").required().describe("Dockerfile or Containerfile text to format. Instruction arguments, heredoc bodies and parser directives are preserved; only layout changes."))
        .param(Param::enumv("instruction_case", ["upper", "lower", "preserve"]).default("upper").describe("Casing for instruction keywords and the AS keyword in a FROM line: 'upper' emits FROM/RUN/COPY (the Docker convention), 'lower' emits from/run/copy, 'preserve' leaves the source casing alone. Default upper."))
        .param(Param::integer("indent").default(4).min(0.0).max(8.0).describe("Spaces prefixed to every line continuation inside a multi-line instruction, from 0 to 8. Default 4."))
        .param(Param::boolean("align_continuations").default(false).describe("Pad continuation lines so the trailing backslashes line up in one column. Default false keeps a single space before each backslash."))
        .param(Param::integer("max_blank_lines").default(1).min(0.0).max(5.0).describe("Maximum consecutive blank lines to keep between instructions, from 0 to 5. Default 1; 0 removes every blank line."))
        .param(Param::boolean("blank_line_between_stages").default(true).describe("Insert one blank line before each build stage after the first, placed above the comments that document that FROM. Default true."))
        .param(Param::boolean("normalize_comments").default(true).describe("Ensure exactly one space after the # of a comment. Banner comments such as #### and top-of-file parser directives are never touched. Default true."))
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct DockerfileFormatter;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/dockerfile-formatter",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Normalize Dockerfile instruction casing, indentation and line continuations",
    skill(
        description = "Format a pasted Dockerfile or Containerfile. Pass the text as `input`; choose instruction keyword casing, continuation indent, backslash alignment, the blank-line cap, stage separation and comment spacing. Instructions are never reordered and their arguments are never rewritten — heredoc bodies and the top-of-file parser directives (# syntax=, # escape=, # check=) are copied through verbatim, and an escape directive switches the continuation character to a backtick. Unknown instructions, dangling line continuations and unterminated heredocs return a line-numbered error instead of partial output. Runs locally.",
        parameters = schema_json()
    ),
)]
impl DockerfileFormatter {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "dockerfile-formatter", |a: Args| {
            gizza_ai_dockerfile_formatter_core::run(
                &a.input,
                &a.instruction_case,
                a.indent,
                a.align_continuations,
                a.max_blank_lines,
                a.blank_line_between_stages,
                a.normalize_comments,
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
    fn schema_has_expected_parameters_and_defaults() {
        let schema: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        let props = schema["properties"].as_object().unwrap();
        for key in [
            "input",
            "instruction_case",
            "indent",
            "align_continuations",
            "max_blank_lines",
            "blank_line_between_stages",
            "normalize_comments",
        ] {
            assert!(props.contains_key(key), "missing {key}");
            assert!(
                props[key]["description"].as_str().unwrap_or_default().len() > 20,
                "{key} needs a usable description"
            );
        }
        assert_eq!(schema["required"], serde_json::json!(["input"]));
        assert_eq!(
            props["instruction_case"]["enum"],
            serde_json::json!(["upper", "lower", "preserve"])
        );
        assert_eq!(props["instruction_case"]["default"], "upper");
        assert_eq!(props["indent"]["default"], 4);
        assert_eq!(props["indent"]["minimum"], 0);
        assert_eq!(props["indent"]["maximum"], 8);
        assert_eq!(props["max_blank_lines"]["default"], 1);
        assert_eq!(props["max_blank_lines"]["minimum"], 0);
        assert_eq!(props["max_blank_lines"]["maximum"], 5);
        assert_eq!(props["align_continuations"]["default"], false);
        assert_eq!(props["blank_line_between_stages"]["default"], true);
        assert_eq!(props["normalize_comments"]["default"], true);
    }

    #[test]
    fn args_defaults_match_descriptor() {
        let a: Args = serde_json::from_str(r#"{"input":"FROM alpine"}"#).unwrap();
        assert_eq!(a.instruction_case, "upper");
        assert_eq!(a.indent, 4);
        assert_eq!(a.max_blank_lines, 1);
        assert!(!a.align_continuations);
        assert!(a.blank_line_between_stages);
        assert!(a.normalize_comments);
    }

    #[test]
    fn manifest_tool_parameters_match_the_descriptor() {
        let manifest: serde_json::Value =
            serde_json::from_str(include_str!("../manifest.json")).unwrap();
        let live: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(manifest["tool"]["parameters"], live);
    }
}
