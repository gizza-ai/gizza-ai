//! gizza-ai/csv-filter — keep CSV rows matching a column condition.
//! Thin wrapper around the core; chat schema single-sourced from descriptor();
//! handler delegates to run_skill. Pure.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_csv_filter_core::filter;
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    data: String,
    condition: String,
    #[serde(default = "default_true")]
    header: bool,
    #[serde(default)]
    delimiter: String,
}
fn default_true() -> bool { true }

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("data").required().describe("The CSV text to filter."))
        .param(Param::string("condition").required().describe("Row condition '<column> <op> <value>'. column is a header name (header=true) or 1-based index; op is one of == != < <= > >= contains. Numeric compare when both sides are numbers, else string; contains is a case-insensitive substring."))
        .param(Param::boolean("header").default(true).describe("Treat the first row as a header (kept, and matchable by name). Default true."))
        .param(Param::string("delimiter").default(",").describe("Field separator: a single char or 'comma'/'tab'/'semicolon'/'pipe'. Default ','."))
}

fn schema_json() -> String { descriptor().to_schema_json() }

#[cfg(target_arch = "wasm32")]
struct CsvFilter;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/csv-filter",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Filter CSV rows by a column condition",
    skill(
        description = "Keep only the CSV rows where a column matches a condition. `condition` is '<column> <op> <value>' with op one of == != < <= > >= contains; the column is a header name (when header=true) or a 1-based index. Comparison is numeric when both the cell and value are numbers, otherwise string; contains is a case-insensitive substring. The header row is preserved.",
        parameters = schema_json()
    )
)]
impl CsvFilter {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "csv-filter", |a: Args| {
            let delim = if a.delimiter.is_empty() { ",".to_string() } else { a.delimiter };
            filter(&a.data, &a.condition, a.header, &delim).map_err(SkillError::InvalidArgs)
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
                    "data":      { "type": "string", "description": "The CSV text to filter." },
                    "condition": { "type": "string", "description": "Row condition '<column> <op> <value>'. column is a header name (header=true) or 1-based index; op is one of == != < <= > >= contains. Numeric compare when both sides are numbers, else string; contains is a case-insensitive substring." },
                    "header":    { "type": "boolean", "default": true, "description": "Treat the first row as a header (kept, and matchable by name). Default true." },
                    "delimiter": { "type": "string", "default": ",", "description": "Field separator: a single char or 'comma'/'tab'/'semicolon'/'pipe'. Default ','." }
                },
                "required": ["data", "condition"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
