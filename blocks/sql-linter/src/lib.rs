//! gizza-ai/sql-linter — chat skill block on the shared tool abstraction.
//!
//! Parses pasted SQL and reports syntax heuristics plus common anti-patterns such
//! as SELECT *, comma joins, missing derived-table aliases, and bare JOINs.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    sql: String,
    #[serde(default)]
    dialect: String,
    #[serde(default)]
    min_severity: String,
    #[serde(default)]
    ignore: String,
    #[serde(default)]
    format: String,
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("sql")
                .required()
                .describe("SQL text to lint. Multiple statements are allowed; comments and string literals are masked before anti-pattern checks so examples inside comments do not trigger findings."),
        )
        .param(
            Param::enumv("dialect", ["generic", "mysql", "postgresql", "sqlite", "tsql"])
                .default("generic")
                .describe("SQL dialect hint. The linter is intentionally heuristic, but the dialect controls details such as MySQL # line-comment masking and is reported in the output."),
        )
        .param(
            Param::enumv("min_severity", ["all", "warning", "error"])
                .default("all")
                .describe("Minimum severity to show. 'all' includes info/warning/error findings; 'warning' hides info-only style hints; 'error' shows only structural syntax errors."),
        )
        .param(
            Param::string("ignore")
                .default("")
                .describe("Optional comma- or space-separated rule codes to suppress, e.g. SELECT-STAR, BARE-JOIN, IMPLICIT-JOIN, SUBQUERY-NO-ALIAS, SYNTAX."),
        )
        .param(
            Param::enumv("format", ["text", "json"])
                .default("text")
                .describe("Output format. 'text' is a human-readable report with line numbers and snippets; 'json' returns summary counts plus a findings array."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct SqlLinter;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/sql-linter",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Lint SQL for syntax issues and query anti-patterns like SELECT *, comma joins, and missing aliases.",
    skill(
        description = "Lint pasted SQL without executing it. Reports structural syntax problems (unbalanced parentheses, unterminated strings/comments, leading/trailing commas) plus common anti-patterns: SELECT *, implicit/comma joins, derived subqueries without aliases, and bare JOINs. Parameters: sql text, dialect hint (generic/mysql/postgresql/sqlite/tsql), min_severity filter, ignore list of rule codes, and text/json output.",
        parameters = schema_json()
    ),
)]
impl SqlLinter {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "sql-linter", |a: Args| {
            gizza_ai_sql_linter_core::lint(
                &a.sql,
                &a.dialect,
                &a.min_severity,
                &a.ignore,
                &a.format,
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
                    "sql": { "type": "string", "description": "SQL text to lint. Multiple statements are allowed; comments and string literals are masked before anti-pattern checks so examples inside comments do not trigger findings." },
                    "dialect": { "type": "string", "enum": ["generic", "mysql", "postgresql", "sqlite", "tsql"], "default": "generic", "description": "SQL dialect hint. The linter is intentionally heuristic, but the dialect controls details such as MySQL # line-comment masking and is reported in the output." },
                    "min_severity": { "type": "string", "enum": ["all", "warning", "error"], "default": "all", "description": "Minimum severity to show. 'all' includes info/warning/error findings; 'warning' hides info-only style hints; 'error' shows only structural syntax errors." },
                    "ignore": { "type": "string", "default": "", "description": "Optional comma- or space-separated rule codes to suppress, e.g. SELECT-STAR, BARE-JOIN, IMPLICIT-JOIN, SUBQUERY-NO-ALIAS, SYNTAX." },
                    "format": { "type": "string", "enum": ["text", "json"], "default": "text", "description": "Output format. 'text' is a human-readable report with line numbers and snippets; 'json' returns summary counts plus a findings array." }
                },
                "required": ["sql"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
