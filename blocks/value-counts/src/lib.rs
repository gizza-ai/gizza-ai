//! gizza-ai/value-counts — chat skill block on the shared tool abstraction.
//! The chat schema is single-sourced from descriptor() (which also drives the
//! CLI); handle() delegates to block_utils::run_skill. Pure → all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_value_counts_core::value_counts;
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    data: String,
    column: String,
    #[serde(default)]
    delimiter: String,
    #[serde(default = "default_sort")]
    sort: String,
    #[serde(default = "default_true")]
    case_sensitive: bool,
    #[serde(default)]
    include_empty: bool,
}
fn default_sort() -> String {
    "count".to_string()
}
fn default_true() -> bool {
    true
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("data").required().describe("The CSV/table text — the first row must be a header."))
        .param(Param::string("column").required().describe("Which column to count: a header name or a 1-based index (e.g. 'status' or '2')."))
        .param(Param::string("delimiter").default(",").describe("Field separator: a single char or 'comma'/'tab'/'semicolon'/'pipe'. Default ','."))
        .param(
            Param::enumv("sort", ["count", "value"])
                .default("count")
                .describe("Row order: 'count' (default) ranks most-frequent-first; 'value' sorts by the value ascending."),
        )
        .param(
            Param::boolean("case_sensitive")
                .default(true)
                .describe("When true (default), 'Apple' and 'apple' count separately; false groups them (first-seen spelling shown)."),
        )
        .param(
            Param::boolean("include_empty")
                .default(false)
                .describe("When true, blank cells are counted as an '(empty)' value (like pandas dropna=False); default false skips them."),
        )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/value-counts",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Count distinct values in a column with percentages",
    skill(
        description = "Count the distinct values in one chosen column of a CSV/table, with each value's count and its percentage of the total, ranked most-frequent-first (the pandas value_counts idiom). `column` is a header name or 1-based index. `sort` is 'count' (default, most frequent first) or 'value' (ascending). `case_sensitive` (default true) controls whether values differing only in case are grouped; `include_empty` (default false) counts blank cells as '(empty)'. Returns a value,count,percent CSV table. Requires a header row. Runs locally.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "value-counts", |a: Args| {
            let delim = if a.delimiter.is_empty() {
                ",".to_string()
            } else {
                a.delimiter
            };
            value_counts(
                &a.data,
                &a.column,
                &delim,
                &a.sort,
                a.case_sensitive,
                a.include_empty,
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
                    "data":      { "type": "string", "description": "The CSV/table text — the first row must be a header." },
                    "column":    { "type": "string", "description": "Which column to count: a header name or a 1-based index (e.g. 'status' or '2')." },
                    "delimiter": { "type": "string", "default": ",", "description": "Field separator: a single char or 'comma'/'tab'/'semicolon'/'pipe'. Default ','." },
                    "sort":      { "type": "string", "enum": ["count", "value"], "default": "count", "description": "Row order: 'count' (default) ranks most-frequent-first; 'value' sorts by the value ascending." },
                    "case_sensitive": { "type": "boolean", "default": true, "description": "When true (default), 'Apple' and 'apple' count separately; false groups them (first-seen spelling shown)." },
                    "include_empty":  { "type": "boolean", "default": false, "description": "When true, blank cells are counted as an '(empty)' value (like pandas dropna=False); default false skips them." }
                },
                "required": ["data", "column"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
