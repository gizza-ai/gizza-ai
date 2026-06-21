//! gizza-ai/csv-stats — per-column summary statistics for a CSV. Thin wrapper;
//! chat schema single-sourced from descriptor(); handler delegates to run_skill.
//! Pure → all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_csv_stats_core::stats;
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    data: String,
    #[serde(default = "default_true")]
    header: bool,
    #[serde(default)]
    delimiter: String,
}
fn default_true() -> bool {
    true
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("data").required().describe("The CSV text."))
        .param(
            Param::boolean("header")
                .default(true)
                .describe("Treat the first row as a header for column names (default true)."),
        )
        .param(
            Param::string("delimiter")
                .default(",")
                .describe("Field separator: a single char or 'comma'/'tab'/'semicolon'/'pipe'. Default ','."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct CsvStats;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/csv-stats",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Per-column summary statistics for a CSV",
    skill(
        description = "Compute per-column summary statistics for a CSV: each column's value count, empty-cell count, number of distinct values, and (for columns whose values are all numeric) min, max, mean, and sum. Returns the stats per column plus the row count. delimiter is a single char or comma/tab/semicolon/pipe. Runs locally.",
        parameters = schema_json()
    ),
)]
impl CsvStats {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "csv-stats", |a: Args| {
            let delim = if a.delimiter.is_empty() { ",".to_string() } else { a.delimiter };
            stats(&a.data, a.header, &delim).map_err(SkillError::InvalidArgs)
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
                    "data": { "type": "string", "description": "The CSV text." },
                    "header": { "type": "boolean", "default": true, "description": "Treat the first row as a header for column names (default true)." },
                    "delimiter": { "type": "string", "default": ",", "description": "Field separator: a single char or 'comma'/'tab'/'semicolon'/'pipe'. Default ','." }
                },
                "required": ["data"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
