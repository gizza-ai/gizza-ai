//! gizza-ai/code-metrics-analyzer — compute LOC, function counts, and approximate complexity.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    source: String,
    #[serde(default = "default_language")]
    language: String,
    #[serde(default = "default_output")]
    output: String,
    #[serde(default = "default_threshold")]
    complexity_threshold: u32,
    #[serde(default = "default_max_functions")]
    max_functions: usize,
    #[serde(default = "default_sort")]
    sort: String,
}

fn default_language() -> String {
    "auto".to_string()
}
fn default_output() -> String {
    "summary".to_string()
}
fn default_threshold() -> u32 {
    10
}
fn default_max_functions() -> usize {
    50
}
fn default_sort() -> String {
    "line".to_string()
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("source")
                .required()
                .multiline()
                .describe("Source code to analyze. Paste one file or snippet; limit 200,000 characters. The analyzer counts physical lines, blank/comment/code lines, function-like declarations, and approximate complexity locally."),
        )
        .param(
            Param::enumv("language", ["auto", "c", "cpp", "csharp", "go", "java", "javascript", "typescript", "kotlin", "lua", "php", "python", "ruby", "rust", "scala", "shell", "sql", "swift"])
                .default("auto")
                .describe("Language hint. Use auto to infer from syntax, or choose a specific language so comment syntax and function patterns match the pasted source."),
        )
        .param(
            Param::enumv("output", ["summary", "functions", "json", "csv"])
                .default("summary")
                .describe("Output format: summary is a readable report, functions is a markdown-style function table, json is structured metrics, and csv lists one function per row."),
        )
        .param(
            Param::integer("complexity_threshold")
                .default(10)
                .min(1.0)
                .max(100.0)
                .describe("Cyclomatic complexity warning threshold. Functions with CCN above this value are counted as over-threshold and marked in function output. Default 10."),
        )
        .param(
            Param::integer("max_functions")
                .default(50)
                .min(0.0)
                .max(500.0)
                .describe("Maximum number of function rows to show in summary/functions/json/csv output. Use 0 to show all detected functions. Totals always include every function."),
        )
        .param(
            Param::enumv("sort", ["line", "complexity", "cognitive", "length", "name"])
                .default("line")
                .describe("Sort order for the function list: source line, descending cyclomatic complexity, descending cognitive complexity, descending length, or name."),
        )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/code-metrics-analyzer",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Measure LOC and complexity in pasted source code",
    skill(
        description = "Analyze one pasted source file or snippet locally and report physical lines, code/comment/blank split, detected functions, approximate cyclomatic complexity, cognitive complexity, Halstead volume, maintainability index, parameter counts, nesting depth, risk bands, and over-threshold functions. Supports auto-detection or an explicit language hint across c, cpp, csharp, go, java, javascript, typescript, kotlin, lua, php, python, ruby, rust, scala, shell, sql, and swift. Outputs readable summary, function table, JSON, or CSV. This is a heuristic lexer, not a full AST parser; it is meant for quick local triage rather than CI-grade repository analysis.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "code-metrics-analyzer", |a: Args| {
            gizza_ai_code_metrics_analyzer_core::run_with_options(
                &a.source,
                &a.language,
                &a.output,
                a.complexity_threshold,
                a.max_functions,
                &a.sort,
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
                    "source": { "type": "string", "description": "Source code to analyze. Paste one file or snippet; limit 200,000 characters. The analyzer counts physical lines, blank/comment/code lines, function-like declarations, and approximate complexity locally." },
                    "language": { "type": "string", "enum": ["auto", "c", "cpp", "csharp", "go", "java", "javascript", "typescript", "kotlin", "lua", "php", "python", "ruby", "rust", "scala", "shell", "sql", "swift"], "default": "auto", "description": "Language hint. Use auto to infer from syntax, or choose a specific language so comment syntax and function patterns match the pasted source." },
                    "output": { "type": "string", "enum": ["summary", "functions", "json", "csv"], "default": "summary", "description": "Output format: summary is a readable report, functions is a markdown-style function table, json is structured metrics, and csv lists one function per row." },
                    "complexity_threshold": { "type": "integer", "default": 10, "minimum": 1, "maximum": 100, "description": "Cyclomatic complexity warning threshold. Functions with CCN above this value are counted as over-threshold and marked in function output. Default 10." },
                    "max_functions": { "type": "integer", "default": 50, "minimum": 0, "maximum": 500, "description": "Maximum number of function rows to show in summary/functions/json/csv output. Use 0 to show all detected functions. Totals always include every function." },
                    "sort": { "type": "string", "enum": ["line", "complexity", "cognitive", "length", "name"], "default": "line", "description": "Sort order for the function list: source line, descending cyclomatic complexity, descending cognitive complexity, descending length, or name." }
                },
                "required": ["source"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
