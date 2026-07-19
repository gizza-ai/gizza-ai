//! gizza-ai/sql-injection-scanner — chat skill block on the shared tool abstraction.
//! The chat schema is single-sourced from descriptor() (which also drives the
//! CLI); handle() delegates to block_utils::run_skill. Pure → all backends.
//! Static heuristic: flags SQL built by concatenation / interpolation / format
//! calls, and bare variables handed to execute()/query(). Never runs the code.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    code: String,
    #[serde(default = "default_language")]
    language: String,
    #[serde(default = "default_min_severity")]
    min_severity: String,
    #[serde(default = "default_format")]
    format: String,
}

fn default_language() -> String {
    "auto".into()
}
fn default_min_severity() -> String {
    "all".into()
}
fn default_format() -> String {
    "text".into()
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("code")
                .required()
                .describe("The source code snippet to scan (one or more lines that build/run SQL)."),
        )
        .param(
            Param::enumv(
                "language",
                [
                    "auto",
                    "python",
                    "php",
                    "javascript",
                    "typescript",
                    "java",
                    "csharp",
                    "go",
                    "ruby",
                ],
            )
            .default("auto")
            .describe(
                "Language hint. auto (default) applies every syntax; a specific language \
                 restricts the concatenation/interpolation patterns to that language (fewer \
                 false positives) and tailors the fix example.",
            ),
        )
        .param(
            Param::enumv("min_severity", ["all", "high"])
                .default("all")
                .describe(
                    "Which findings to report: all (default) shows high + medium; high shows \
                     only high-severity findings.",
                ),
        )
        .param(
            Param::enumv("format", ["text", "json"])
                .default("text")
                .describe(
                    "Output format: text (default) a readable report, or json (structured findings).",
                ),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/sql-injection-scanner",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Flag SQL built by string concatenation or interpolation instead of parameters",
    skill(
        description = "Statically scan a code snippet for SQL-injection-prone query construction. Flags SQL assembled from variables instead of bound parameters: string concatenation (\"… \" + name, PHP . $name), string interpolation (Python f-strings, JS/TS template literals ${…}, C# $\"…\", Ruby #{…}), and format/printf building (\"…\".format(), \"…\" % v, sprintf/Sprintf/String.Format). Also warns (medium) when a bare variable is passed to execute()/query(). Each finding reports line, column, severity, a rule id, and the offending line; parameterized calls like execute(sql, params) are recognized as safe. Params: code (the snippet), language (auto|python|php|javascript|typescript|java|csharp|go|ruby — scopes the patterns), min_severity (all|high), format (text|json). It never runs the code or connects to a database — a heuristic aid, not a guarantee. Runs locally.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "sql-injection-scanner", |a: Args| {
            gizza_ai_sql_injection_scanner_core::run(
                &a.code,
                &a.language,
                &a.min_severity,
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
                    "code": {
                        "type": "string",
                        "description": "The source code snippet to scan (one or more lines that build/run SQL)."
                    },
                    "language": {
                        "type": "string",
                        "enum": ["auto", "python", "php", "javascript", "typescript", "java", "csharp", "go", "ruby"],
                        "default": "auto",
                        "description": "Language hint. auto (default) applies every syntax; a specific language restricts the concatenation/interpolation patterns to that language (fewer false positives) and tailors the fix example."
                    },
                    "min_severity": {
                        "type": "string",
                        "enum": ["all", "high"],
                        "default": "all",
                        "description": "Which findings to report: all (default) shows high + medium; high shows only high-severity findings."
                    },
                    "format": {
                        "type": "string",
                        "enum": ["text", "json"],
                        "default": "text",
                        "description": "Output format: text (default) a readable report, or json (structured findings)."
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
