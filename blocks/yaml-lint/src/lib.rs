//! gizza-ai/yaml-lint — YAML syntax and style linter on the shared tool abstraction.
//! The chat schema is single-sourced from descriptor() (which also drives the
//! CLI); handle() delegates to block_utils::run_skill. Pure → all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    input: String,
    #[serde(default = "default_preset")]
    preset: String,
    #[serde(default = "default_indent_spaces")]
    indent_spaces: usize,
    #[serde(default = "default_max_line_length")]
    max_line_length: usize,
    #[serde(default)]
    disable: String,
    #[serde(default)]
    strict_warnings: bool,
    #[serde(default = "default_report_format")]
    report_format: String,
}

fn default_preset() -> String {
    "default".to_string()
}
fn default_indent_spaces() -> usize {
    2
}
fn default_max_line_length() -> usize {
    80
}
fn default_report_format() -> String {
    "report".to_string()
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("input").required().describe("YAML document to lint. Multi-document streams are supported and no data leaves the local runtime."))
        .param(Param::enumv("preset", ["relaxed", "default", "strict"]).default("default").describe("Rule preset. relaxed checks syntax, duplicate keys and indentation; default adds common style and portability warnings; strict adds document-start, key-ordering and empty-value checks."))
        .param(Param::integer("indent_spaces").default(2).min(1.0).max(8.0).describe("Expected indentation width in spaces, from 1 to 8. Lines indented by a non-multiple are reported."))
        .param(Param::integer("max_line_length").default(80).min(0.0).max(1000.0).describe("Maximum line length in characters. Set to 0 to disable the line-length rule."))
        .param(Param::string("disable").default("").describe("Comma, space or newline separated rule ids to disable, such as truthy, line-length or comments."))
        .param(Param::boolean("strict_warnings").default(false).describe("Promote every warning to an error, matching the strict CI convention used by many YAML linters."))
        .param(Param::enumv("report_format", ["report", "json"]).default("report").describe("Output format: report returns human-readable line:column findings; json returns a machine-readable object for CI."))
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct YamlLint;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/yaml-lint",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Lint YAML for syntax, duplicate keys and style traps",
    skill(
        description = "Lint a pasted YAML document for parser errors, duplicate mapping keys, tabs and indentation drift, line length, trailing whitespace, comment and colon spacing, truthy/octal-looking scalars, multi-document stream counts and optional strict rules. Choose relaxed/default/strict presets, disable individual rules, adjust indentation and line-length limits, and return either a human report or JSON for CI.",
        parameters = schema_json()
    ),
)]
impl YamlLint {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "yaml-lint", |a: Args| {
            gizza_ai_yaml_lint_core::run(
                &a.input,
                &a.preset,
                a.indent_spaces,
                a.max_line_length,
                &a.disable,
                a.strict_warnings,
                &a.report_format,
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
            "preset",
            "indent_spaces",
            "max_line_length",
            "disable",
            "strict_warnings",
            "report_format",
        ] {
            assert!(props.contains_key(key), "missing {key}");
            assert!(props[key]["description"].as_str().unwrap_or_default().len() > 20);
        }
        assert_eq!(schema["required"], serde_json::json!(["input"]));
        assert_eq!(
            props["preset"]["enum"],
            serde_json::json!(["relaxed", "default", "strict"])
        );
        assert_eq!(props["preset"]["default"], "default");
        assert_eq!(props["indent_spaces"]["minimum"], 1);
        assert_eq!(props["indent_spaces"]["maximum"], 8);
        assert_eq!(props["max_line_length"]["minimum"], 0);
        assert_eq!(props["max_line_length"]["maximum"], 1000);
        assert_eq!(props["strict_warnings"]["default"], false);
        assert_eq!(
            props["report_format"]["enum"],
            serde_json::json!(["report", "json"])
        );
        assert_eq!(props["report_format"]["default"], "report");
    }
}
