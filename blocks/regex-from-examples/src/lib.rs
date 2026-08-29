//! gizza-ai/regex-from-examples — infer a regex from positive and negative examples.
//!
//! The chat schema is single-sourced from `descriptor()` so the CLI, manifest,
//! and generated page stay aligned. The inference itself is deterministic and
//! lives in the core crate; this block only parses arguments and wraps the text
//! result for Wafer.

#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    examples: String,
    #[serde(default)]
    negatives: String,
    #[serde(default = "default_separator")]
    separator: String,
    #[serde(default = "default_strategy")]
    strategy: String,
    #[serde(default = "default_quantifiers")]
    quantifiers: String,
    #[serde(default = "default_flavor")]
    flavor: String,
    #[serde(default = "default_true")]
    anchors: bool,
    #[serde(default)]
    case_insensitive: bool,
    #[serde(default)]
    capture_groups: bool,
    #[serde(default = "default_output")]
    output: String,
    #[serde(default = "default_max_alternatives")]
    max_alternatives: f64,
}

fn default_separator() -> String { "newline".into() }
fn default_strategy() -> String { "auto".into() }
fn default_quantifiers() -> String { "range".into() }
fn default_flavor() -> String { "rust".into() }
fn default_output() -> String { "pattern".into() }
fn default_true() -> bool { true }
fn default_max_alternatives() -> f64 { 50.0 }

/// Single source for chat schema, CLI parameters, generated page controls, and
/// manifest sync. The tool takes plain text fields rather than a file/url input.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("examples")
                .required()
                .multiline()
                .placeholder("2024-01-15\n2023-11-02\n1999-12-31")
                .describe("Positive example strings the inferred regular expression must match. Blank entries are ignored."),
        )
        .param(
            Param::string("negatives")
                .multiline()
                .placeholder("2024/01/15\nnot-a-date")
                .describe("Optional counter-examples the inferred pattern should reject, split with the same separator."),
        )
        .param(
            Param::enumv("separator", ["newline", "comma", "tab", "semicolon", "space"])
                .default("newline")
                .describe("How to split examples and negatives before inference."),
        )
        .param(
            Param::enumv("strategy", ["auto", "generalize", "alternation", "character-class"])
                .default("auto")
                .describe("Inference strategy. Auto tries structural generalization first, then literal alternation when negatives require it."),
        )
        .param(
            Param::enumv("quantifiers", ["range", "open", "loose"])
                .default("range")
                .describe("How observed run lengths become regex quantifiers: exact/range, open-ended minimum, or loose +/*/? tokens."),
        )
        .param(
            Param::enumv("flavor", ["rust", "pcre", "python", "javascript", "posix"])
                .default("rust")
                .describe("Regex syntax flavor to emit. Verification always compiles the equivalent Rust regex."),
        )
        .param(
            Param::boolean("anchors")
                .default(true)
                .describe("Wrap the result so it must match the whole string (recommended for validators)."),
        )
        .param(
            Param::boolean("case_insensitive")
                .default(false)
                .describe("Treat ASCII letters case-insensitively and emit the corresponding flag or expanded classes for the chosen flavor."),
        )
        .param(
            Param::boolean("capture_groups")
                .default(false)
                .describe("Use capturing groups around variable fields instead of non-capturing groups where the flavor supports them."),
        )
        .param(
            Param::enumv("output", ["pattern", "report", "json"])
                .default("pattern")
                .describe("Return only the regex, a readable explanation/verification report, or structured JSON."),
        )
        .param(
            Param::number("max_alternatives")
                .default(50.0)
                .min(1.0)
                .max(500.0)
                .describe("Maximum literal alternatives to emit before failing with a simpler input request (1-500, default 50)."),
        )
}

fn schema_json() -> String { descriptor().to_schema_json() }

#[cfg(target_arch = "wasm32")]
struct RegexFromExamples;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/regex-from-examples",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Infer a verified regex from positive and negative examples",
    skill(
        description = "Infer a deterministic regular expression from positive examples, optionally rejecting negative examples. Choose separators, inference strategy, quantifier style, regex flavor, anchoring, case sensitivity, capture groups, and output format. The result is compiled and checked against the samples before it is returned; it is not ML and does not promise a globally minimal regex.",
        parameters = schema_json()
    ),
)]
impl RegexFromExamples {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "regex-from-examples", |a: Args| {
            gizza_ai_regex_from_examples_core::render(
                &a.examples,
                &a.negatives,
                &a.separator,
                &a.strategy,
                &a.quantifiers,
                &a.flavor,
                a.anchors,
                a.case_insensitive,
                a.capture_groups,
                &a.output,
                a.max_alternatives,
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
        let schema: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        let props = schema["properties"].as_object().unwrap();
        assert_eq!(schema["required"], serde_json::json!(["examples"]));
        assert_eq!(props["strategy"]["enum"], serde_json::json!(["auto", "generalize", "alternation", "character-class"]));
        assert_eq!(props["flavor"]["enum"], serde_json::json!(["rust", "pcre", "python", "javascript", "posix"]));
        assert_eq!(props["output"]["enum"], serde_json::json!(["pattern", "report", "json"]));
        assert_eq!(props["anchors"]["default"], true);
        assert_eq!(props["max_alternatives"]["minimum"], 1);
        assert_eq!(props["max_alternatives"]["maximum"], 500);
        assert_eq!(schema["additionalProperties"], false);
    }
}
