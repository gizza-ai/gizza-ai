//! gizza-ai/csv-sort — sort CSV/TSV rows by one or more columns, ascending or
//! descending, numeric-aware or lexical. Thin wrapper; chat schema single-sourced
//! from descriptor(); handler delegates to run_skill. Pure → all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_csv_sort_core::sort;
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    data: String,
    columns: String,
    #[serde(default = "default_asc")]
    order: String,
    #[serde(default = "default_auto")]
    numeric: String,
    #[serde(default)]
    case_sensitive: bool,
    #[serde(default = "default_true")]
    header: bool,
    #[serde(default)]
    delimiter: String,
}
fn default_true() -> bool {
    true
}
fn default_asc() -> String {
    "asc".into()
}
fn default_auto() -> String {
    "auto".into()
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("data").required().describe("The CSV text (with a header row unless header=false)."))
        .param(Param::string("columns").required().describe(
            "Sort column(s), comma-separated in priority order: column names (when header=true) or 1-based indices. Each may carry a ':asc'/':desc' suffix to set its own direction (else the global order applies). e.g. 'age' or 'dept:asc,salary:desc'.",
        ))
        .param(
            Param::enumv("order", ["asc", "desc"])
                .default("asc")
                .describe("Default sort direction for columns without their own ':asc'/':desc' suffix. Default 'asc'."),
        )
        .param(
            Param::enumv("numeric", ["auto", "number", "text"])
                .default("auto")
                .describe("Ordering: 'auto' sorts a column numerically when every value is a number (so 2 < 10), else lexically; 'number' forces numeric; 'text' forces lexical. Default 'auto'."),
        )
        .param(
            Param::boolean("case_sensitive")
                .default(false)
                .describe("For text ordering, make A–Z case-sensitive (uppercase sorts before lowercase). Default false."),
        )
        .param(
            Param::boolean("header")
                .default(true)
                .describe("Treat the first row as a header (kept on top; columns can be named). With false, sort every row and address columns by 1-based index. Default true."),
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
struct CsvSort;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/csv-sort",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Sort CSV rows by one or more columns",
    skill(
        description = "Sort the rows of a CSV/TSV by one or more columns. `columns` is a comma-separated priority list of column names (when header=true) or 1-based indices; each may carry a ':asc'/':desc' suffix for its own direction, else `order` (asc/desc) applies. `numeric` is auto (numeric when a column is all numbers, so 2<10, else lexical), number, or text. case_sensitive controls text ordering; the header row stays on top. delimiter is a single char or comma/tab/semicolon/pipe. Runs locally.",
        parameters = schema_json()
    ),
)]
impl CsvSort {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "csv-sort", |a: Args| {
            let delim = if a.delimiter.is_empty() { ",".to_string() } else { a.delimiter };
            sort(&a.data, &a.columns, &a.order, &a.numeric, a.case_sensitive, a.header, &delim)
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
                    "data": { "type": "string", "description": "The CSV text (with a header row unless header=false)." },
                    "columns": { "type": "string", "description": "Sort column(s), comma-separated in priority order: column names (when header=true) or 1-based indices. Each may carry a ':asc'/':desc' suffix to set its own direction (else the global order applies). e.g. 'age' or 'dept:asc,salary:desc'." },
                    "order": { "type": "string", "enum": ["asc", "desc"], "default": "asc", "description": "Default sort direction for columns without their own ':asc'/':desc' suffix. Default 'asc'." },
                    "numeric": { "type": "string", "enum": ["auto", "number", "text"], "default": "auto", "description": "Ordering: 'auto' sorts a column numerically when every value is a number (so 2 < 10), else lexically; 'number' forces numeric; 'text' forces lexical. Default 'auto'." },
                    "case_sensitive": { "type": "boolean", "default": false, "description": "For text ordering, make A–Z case-sensitive (uppercase sorts before lowercase). Default false." },
                    "header": { "type": "boolean", "default": true, "description": "Treat the first row as a header (kept on top; columns can be named). With false, sort every row and address columns by 1-based index. Default true." },
                    "delimiter": { "type": "string", "default": ",", "description": "Field separator: a single char or 'comma'/'tab'/'semicolon'/'pipe'. Default ','." }
                },
                "required": ["data", "columns"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
