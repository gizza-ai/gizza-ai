//! gizza-ai/csv-to-table — convert CSV into a Markdown or HTML table. Thin
//! wrapper; chat schema single-sourced from descriptor(); handler delegates to
//! run_skill. Pure → all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_csv_to_table_core::{to_table, Format};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    data: String,
    #[serde(default = "default_format")]
    format: String,
    #[serde(default = "default_true")]
    header: bool,
    #[serde(default)]
    delimiter: String,
}
fn default_format() -> String {
    "markdown".to_string()
}
fn default_true() -> bool {
    true
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("data").required().describe("The CSV text."))
        .param(
            Param::enumv("format", ["markdown", "html"]).default("markdown").describe(
                "Output table format: markdown (default) or html.",
            ),
        )
        .param(
            Param::boolean("header")
                .default(true)
                .describe("Treat the first row as the header (default true); otherwise 'Column N' headers are generated."),
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
struct CsvToTable;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/csv-to-table",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Convert CSV into a Markdown or HTML table",
    skill(
        description = "Convert CSV data into a Markdown or HTML table. format=markdown (default) produces a GitHub-style pipe table (cells escaped); format=html produces a <table> with thead/tbody (cells HTML-escaped). header=true (default) uses the first row as the header, otherwise 'Column N' headers are generated. delimiter is a single char or comma/tab/semicolon/pipe. Runs locally.",
        parameters = schema_json()
    ),
)]
impl CsvToTable {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "csv-to-table", |a: Args| {
            let fmt = Format::parse(&a.format).map_err(SkillError::InvalidArgs)?;
            let delim = if a.delimiter.is_empty() { ",".to_string() } else { a.delimiter };
            to_table(&a.data, fmt, a.header, &delim).map_err(SkillError::InvalidArgs)
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
                    "format": { "type": "string", "enum": ["markdown", "html"], "default": "markdown", "description": "Output table format: markdown (default) or html." },
                    "header": { "type": "boolean", "default": true, "description": "Treat the first row as the header (default true); otherwise 'Column N' headers are generated." },
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
