//! gizza-ai/csv-query — run a small SQL-ish query over CSV.
//! Thin wrapper around the core; chat schema single-sourced from descriptor();
//! handler delegates to run_skill. Pure.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_csv_query_core::query;
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    data: String,
    query: String,
    #[serde(default)]
    delimiter: String,
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("data").required().describe("The CSV text (first row must be a header)."))
        .param(Param::string("query").required().describe("A SQL-ish query: SELECT <cols|*> [WHERE <col op value>] [ORDER BY <col> [ASC|DESC]] [LIMIT n]. Columns are header names or 1-based indices; op is == != < <= > >= contains. (Aggregates/GROUP BY are out of scope — use csv-group-by / csv-pivot.)"))
        .param(Param::string("delimiter").default(",").describe("Field separator: a single char or 'comma'/'tab'/'semicolon'/'pipe'. Default ','."))
}

fn schema_json() -> String { descriptor().to_schema_json() }

#[cfg(target_arch = "wasm32")]
struct CsvQuery;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/csv-query",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Run a SQL-ish query over CSV",
    skill(
        description = "Query a CSV with a small SQL-ish language: SELECT <columns or *> [WHERE <col op value>] [ORDER BY <col> [ASC|DESC]] [LIMIT n]. Columns are header names or 1-based indices; WHERE op is one of == != < <= > >= contains (numeric compare when both sides are numbers, else string). Returns the projected/filtered/sorted CSV. For aggregation use csv-group-by or csv-pivot.",
        parameters = schema_json()
    )
)]
impl CsvQuery {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "csv-query", |a: Args| {
            let delim = if a.delimiter.is_empty() { ",".to_string() } else { a.delimiter };
            query(&a.data, &a.query, &delim).map_err(SkillError::InvalidArgs)
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
                    "data":      { "type": "string", "description": "The CSV text (first row must be a header)." },
                    "query":     { "type": "string", "description": "A SQL-ish query: SELECT <cols|*> [WHERE <col op value>] [ORDER BY <col> [ASC|DESC]] [LIMIT n]. Columns are header names or 1-based indices; op is == != < <= > >= contains. (Aggregates/GROUP BY are out of scope — use csv-group-by / csv-pivot.)" },
                    "delimiter": { "type": "string", "default": ",", "description": "Field separator: a single char or 'comma'/'tab'/'semicolon'/'pipe'. Default ','." }
                },
                "required": ["data", "query"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
