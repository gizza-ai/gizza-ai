//! gizza-ai/config-file-validator — validate pasted JSON, YAML, TOML, INI or XML.
//! The chat schema is single-sourced from descriptor() (which also drives the
//! CLI); handle() delegates to block_utils::run_skill.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

fn default_format() -> String {
    "auto".to_string()
}
fn default_report_format() -> String {
    "report".to_string()
}
fn default_context_lines() -> usize {
    2
}

#[derive(Deserialize)]
struct Args {
    input: String,
    #[serde(default = "default_format")]
    format: String,
    #[serde(default)]
    strict: bool,
    #[serde(default = "default_report_format")]
    report_format: String,
    #[serde(default = "default_context_lines")]
    context_lines: usize,
}

/// Single source for the chat schema (and CLI). Edit the params to match the
/// tool's real inputs — e.g. `.param(Param::enumv("mode", ["a","b"]).default("a"))`,
/// `.param(Param::integer("n").min(1.0))`. Use Input::Image/Video/Document/File
/// for tools that take a url/ref media input (see image-resize / web-fetch).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("input").required().describe("The config file contents to validate, pasted as text. Supports JSON, YAML (including multi-document streams), TOML, INI-style key/value files and XML. Up to 1 MiB."))
        .param(Param::enumv("format", ["auto", "json", "yaml", "toml", "ini", "xml"]).default("auto").describe("Which parser to use. 'auto' ranks likely formats from the text and keeps the first parser that succeeds; choose a specific format to get that parser's exact line and column error."))
        .param(Param::boolean("strict").default(false).describe("Add portability warnings after syntax passes: duplicate JSON keys, tab indentation, BOMs and mixed line endings. Warnings do not make the file invalid. Default false."))
        .param(Param::enumv("report_format", ["report", "json"]).default("report").describe("Output style. 'report' is a human-readable diagnostic with line/column and source context; 'json' returns a machine-readable object with valid, format, counts and diagnostics."))
        .param(Param::integer("context_lines").default(2).min(0.0).max(10.0).describe("Number of source lines to show before and after each diagnostic in the human report, from 0 to 10. Default 2. Ignored for report_format=json."))
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct ConfigFileValidator;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/config-file-validator",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Validate JSON, YAML, TOML, INI or XML config syntax",
    skill(
        description = "Validate the syntax of a pasted config file and report line/column diagnostics. Pass the file text as `input` and choose `format=auto` (default) or force `json`, `yaml`, `toml`, `ini` or `xml`. The human `report` output names the detected/specified format, says whether the file is valid, and shows parser errors with source context and targeted hints; `report_format=json` returns a machine-readable diagnostic object. `strict=true` adds portability warnings after syntax passes, including duplicate JSON keys, tab indentation, BOMs and mixed line endings. Everything runs locally with pure parsers; no network or file access.",
        parameters = schema_json()
    ),
)]
impl ConfigFileValidator {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "config-file-validator", |a: Args| {
            gizza_ai_config_file_validator_core::validate(
                &a.input,
                &a.format,
                a.strict,
                &a.report_format,
                a.context_lines,
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
            "format",
            "strict",
            "report_format",
            "context_lines",
        ] {
            assert!(props.contains_key(key), "missing {key}");
            assert!(props[key]["description"].as_str().unwrap_or_default().len() > 20);
        }
        assert_eq!(schema["required"], serde_json::json!(["input"]));
        assert_eq!(props["format"]["default"], "auto");
        assert_eq!(props["report_format"]["default"], "report");
        assert_eq!(props["context_lines"]["minimum"], 0);
        assert_eq!(props["context_lines"]["maximum"], 10);
    }

    #[test]
    fn args_defaults_match_descriptor() {
        let a: Args = serde_json::from_str(r#"{"input":"{\"a\":1}"}"#).unwrap();
        assert_eq!(a.format, "auto");
        assert!(!a.strict);
        assert_eq!(a.report_format, "report");
        assert_eq!(a.context_lines, 2);
    }
}
