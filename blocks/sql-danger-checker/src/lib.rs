//! gizza-ai/sql-danger-checker — chat skill block on the shared tool abstraction.
//! The chat schema is single-sourced from descriptor() (which also drives the
//! CLI); handle() delegates to block_utils::run_skill. Pure → all backends.
//! Static heuristic: scans raw SQL for destructive statements (DROP/TRUNCATE,
//! UPDATE/DELETE without WHERE, destructive ALTER) BEFORE they run. It never
//! connects to a database and never executes anything.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    sql: String,
    #[serde(default = "default_dialect")]
    dialect: String,
    #[serde(default = "default_min_severity")]
    min_severity: String,
    #[serde(default = "default_strict")]
    strict: String,
    #[serde(default)]
    allow: String,
    #[serde(default = "default_format")]
    format: String,
}

fn default_dialect() -> String {
    "generic".into()
}
fn default_min_severity() -> String {
    "all".into()
}
fn default_strict() -> String {
    "false".into()
}
fn default_format() -> String {
    "text".into()
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("sql")
                .required()
                .describe("The raw SQL to scan (one or more statements separated by ;). Not executed."),
        )
        .param(
            Param::enumv(
                "dialect",
                ["generic", "mysql", "postgresql", "sqlite", "tsql"],
            )
            .default("generic")
            .describe(
                "SQL dialect. generic (default) uses standard lexing; mysql also treats # as a \
                 line comment. All dialects share the same destructive-statement rules.",
            ),
        )
        .param(
            Param::enumv("min_severity", ["all", "medium", "high", "critical"])
                .default("all")
                .describe(
                    "Lowest severity to report: all (default) shows everything, or medium/high/\
                     critical to raise the floor. critical = DROP TABLE/DATABASE, TRUNCATE.",
                ),
        )
        .param(
            Param::boolean("strict")
                .default(false)
                .describe(
                    "When true, also flag guarded DELETE/UPDATE (they DO have a WHERE) as a low \
                     'confirm before running' finding, so every destructive statement is surfaced.",
                ),
        )
        .param(
            Param::string("allow")
                .default("")
                .describe(
                    "Comma-separated categories to suppress when you know they are intentional: \
                     drop, truncate, alter, delete, update. Empty (default) reports all.",
                ),
        )
        .param(
            Param::enumv("format", ["text", "json"])
                .default("text")
                .describe("Output format: text (default) a readable report, or json (structured findings)."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/sql-danger-checker",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Scan raw SQL for destructive statements (DROP, TRUNCATE, WHERE-less UPDATE/DELETE) before you run them",
    skill(
        description = "Statically scan raw SQL for destructive statements before you execute them. Flags DROP DATABASE/SCHEMA/TABLE and TRUNCATE (critical); DROP of other objects and ALTER … DROP (high); other ALTER schema changes (medium); and UPDATE/DELETE with no WHERE clause — or a trivially-true one like WHERE 1=1 — which touch every row (high). It splits multiple statements on ; and is comment- and string-aware: a DROP inside a -- / /* */ comment or a 'string literal' is ignored, and a ; inside a string does not split a statement. Each finding reports the statement number, line, severity, a rule id, and the offending statement. Params: sql (the statements), dialect (generic|mysql|postgresql|sqlite|tsql), min_severity (all|medium|high|critical), strict (true also flags guarded DELETE/UPDATE for confirmation), allow (comma list of drop|truncate|alter|delete|update to suppress), format (text|json). It never connects to a database or runs anything — a heuristic pre-flight check, not a guarantee. Runs locally.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "sql-danger-checker", |a: Args| {
            gizza_ai_sql_danger_checker_core::run(
                &a.sql,
                &a.dialect,
                &a.min_severity,
                &a.strict,
                &a.allow,
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
                    "sql": {
                        "type": "string",
                        "description": "The raw SQL to scan (one or more statements separated by ;). Not executed."
                    },
                    "dialect": {
                        "type": "string",
                        "enum": ["generic", "mysql", "postgresql", "sqlite", "tsql"],
                        "default": "generic",
                        "description": "SQL dialect. generic (default) uses standard lexing; mysql also treats # as a line comment. All dialects share the same destructive-statement rules."
                    },
                    "min_severity": {
                        "type": "string",
                        "enum": ["all", "medium", "high", "critical"],
                        "default": "all",
                        "description": "Lowest severity to report: all (default) shows everything, or medium/high/critical to raise the floor. critical = DROP TABLE/DATABASE, TRUNCATE."
                    },
                    "strict": {
                        "type": "boolean",
                        "default": false,
                        "description": "When true, also flag guarded DELETE/UPDATE (they DO have a WHERE) as a low 'confirm before running' finding, so every destructive statement is surfaced."
                    },
                    "allow": {
                        "type": "string",
                        "default": "",
                        "description": "Comma-separated categories to suppress when you know they are intentional: drop, truncate, alter, delete, update. Empty (default) reports all."
                    },
                    "format": {
                        "type": "string",
                        "enum": ["text", "json"],
                        "default": "text",
                        "description": "Output format: text (default) a readable report, or json (structured findings)."
                    }
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
