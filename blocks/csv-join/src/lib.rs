//! gizza-ai/csv-join — join two CSVs on a key column, SQL-style (inner / left /
//! right / full outer). Thin wrapper; chat schema single-sourced from
//! descriptor(); handler delegates to run_skill. Pure → all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_csv_join_core::join;
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    left: String,
    right: String,
    left_key: String,
    #[serde(default)]
    right_key: String,
    #[serde(default = "default_inner")]
    join_type: String,
    #[serde(default)]
    delimiter: String,
    #[serde(default = "default_true")]
    case_sensitive: bool,
}
fn default_true() -> bool {
    true
}
fn default_inner() -> String {
    "inner".into()
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("left").required().describe("The left (first) CSV text; the first row is its header."))
        .param(Param::string("right").required().describe("The right (second) CSV text; the first row is its header."))
        .param(Param::string("left_key").required().describe(
            "Key column in the LEFT CSV to join on — a header name or a 1-based column index.",
        ))
        .param(Param::string("right_key").default("").describe(
            "Key column in the RIGHT CSV — a header name or 1-based index. Matched on VALUES, so it may have a different name than left_key. Blank reuses left_key's reference.",
        ))
        .param(
            Param::enumv("join_type", ["inner", "left", "right", "outer"])
                .default("inner")
                .describe("Which rows to keep: 'inner' only matching keys; 'left' all left rows (unmatched right cells blank); 'right' all right rows; 'outer' all rows from both sides. Default 'inner'."),
        )
        .param(
            Param::string("delimiter")
                .default(",")
                .describe("Field separator: a single char or 'comma'/'tab'/'semicolon'/'pipe'. Default ','."),
        )
        .param(
            Param::boolean("case_sensitive")
                .default(true)
                .describe("Match key VALUES case-sensitively. false makes 'A1' and 'a1' match. Default true."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct CsvJoin;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/csv-join",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Join two CSVs on a key column, SQL-style",
    skill(
        description = "Join two CSV inputs on a key column, SQL-style. `join_type` is inner (only matching keys), left (all left rows), right (all right rows), or outer (all rows from both sides); unmatched cells are blank. `left_key`/`right_key` are header names or 1-based indices and are matched on VALUES, so the two key columns may have different names (blank right_key reuses left_key). Output = the full left header then every non-key right column (name collisions get a '_right' suffix). Duplicate keys yield a Cartesian product. delimiter is a single char or comma/tab/semicolon/pipe. Runs locally.",
        parameters = schema_json()
    ),
)]
impl CsvJoin {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "csv-join", |a: Args| {
            let delim = if a.delimiter.is_empty() { ",".to_string() } else { a.delimiter };
            join(
                &a.left,
                &a.right,
                &a.left_key,
                &a.right_key,
                &a.join_type,
                &delim,
                a.case_sensitive,
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
                    "left": { "type": "string", "description": "The left (first) CSV text; the first row is its header." },
                    "right": { "type": "string", "description": "The right (second) CSV text; the first row is its header." },
                    "left_key": { "type": "string", "description": "Key column in the LEFT CSV to join on — a header name or a 1-based column index." },
                    "right_key": { "type": "string", "default": "", "description": "Key column in the RIGHT CSV — a header name or 1-based index. Matched on VALUES, so it may have a different name than left_key. Blank reuses left_key's reference." },
                    "join_type": { "type": "string", "enum": ["inner", "left", "right", "outer"], "default": "inner", "description": "Which rows to keep: 'inner' only matching keys; 'left' all left rows (unmatched right cells blank); 'right' all right rows; 'outer' all rows from both sides. Default 'inner'." },
                    "delimiter": { "type": "string", "default": ",", "description": "Field separator: a single char or 'comma'/'tab'/'semicolon'/'pipe'. Default ','." },
                    "case_sensitive": { "type": "boolean", "default": true, "description": "Match key VALUES case-sensitively. false makes 'A1' and 'a1' match. Default true." }
                },
                "required": ["left", "right", "left_key"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
