//! gizza-ai/sql-formatter — pretty-print / standardize a SQL query. Chat schema
//! single-sourced from descriptor(); handler delegates to run_skill. Pure → all
//! backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_sql_formatter_core::{format, KeywordCase};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    sql: String,
    #[serde(default = "default_indent")]
    indent: u64,
    #[serde(default = "default_case")]
    keyword_case: String,
}

fn default_indent() -> u64 {
    2
}
fn default_case() -> String {
    "upper".to_string()
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("sql").required().describe("The SQL query to format."))
        .param(
            Param::integer("indent")
                .min(0.0)
                .max(8.0)
                .default(2)
                .describe("Spaces of indentation per nesting level (0-8, default 2)."),
        )
        .param(
            Param::enumv("keyword_case", ["upper", "lower", "preserve"])
                .default("upper")
                .describe(
                    "Casing for SQL keywords: upper (default), lower, or preserve (keep as written).",
                ),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct SqlFormatter;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/sql-formatter",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Pretty-print and standardize a SQL query's formatting",
    skill(
        description = "Pretty-print (beautify) and standardize a SQL query — major clauses (SELECT/FROM/WHERE/JOIN/GROUP BY/ORDER BY/…) on their own lines, select-list and column items one per line, AND/OR conditions broken and indented, function calls kept tight, and string literals/comments preserved verbatim. indent sets spaces per level (0-8, default 2); keyword_case = upper (default), lower, or preserve. Dialect-agnostic and forgiving — it reformats the token stream rather than rejecting unusual SQL. Runs locally.",
        parameters = schema_json()
    ),
)]
impl SqlFormatter {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "sql-formatter", |a: Args| {
            let case = KeywordCase::parse(&a.keyword_case).map_err(SkillError::InvalidArgs)?;
            format(&a.sql, a.indent as usize, case).map_err(SkillError::InvalidArgs)
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
                    "sql":          { "type": "string", "description": "The SQL query to format." },
                    "indent":       { "type": "integer", "minimum": 0, "maximum": 8, "default": 2, "description": "Spaces of indentation per nesting level (0-8, default 2)." },
                    "keyword_case": { "type": "string", "enum": ["upper", "lower", "preserve"], "default": "upper", "description": "Casing for SQL keywords: upper (default), lower, or preserve (keep as written)." }
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
