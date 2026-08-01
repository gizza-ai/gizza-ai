//! gizza-ai/query-result-formatter — render raw query result rows (JSON array,
//! CSV, or TSV) into a clean Markdown or ASCII table. Thin wrapper; chat schema
//! single-sourced from descriptor(); handler delegates to run_skill. Pure → all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_query_result_formatter_core::{format_result, Align, Format, InputFormat};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    data: String,
    #[serde(default = "default_input_format")]
    input_format: String,
    #[serde(default = "default_format")]
    format: String,
    #[serde(default = "default_true")]
    header: bool,
    #[serde(default = "default_align")]
    align: String,
    #[serde(default)]
    null_text: String,
}
fn default_input_format() -> String {
    "auto".to_string()
}
fn default_format() -> String {
    "markdown".to_string()
}
fn default_align() -> String {
    "left".to_string()
}
fn default_true() -> bool {
    true
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("data")
                .required()
                .describe("The raw query result rows: a JSON array of row objects (or arrays), CSV, or TSV text."),
        )
        .param(
            Param::enumv("input_format", ["auto", "json", "csv", "tsv", "sql"]).default("auto").describe(
                "Shape of the input: auto (default — sniffs JSON vs SQL-CLI-table vs TSV vs CSV), json, csv, tsv, or sql (a psql/MySQL/SQLite result table pasted from a database shell — pipe columns, +---+/---+--- rules and the (N rows) footer are handled).",
            ),
        )
        .param(
            Param::enumv("format", ["markdown", "ascii"]).default("markdown").describe(
                "Output table format: markdown (default, GitHub-style pipe table) or ascii (box-drawing grid).",
            ),
        )
        .param(
            Param::boolean("header")
                .default(true)
                .describe("For CSV/TSV and JSON arrays-of-arrays, treat the first row as the header (default true); otherwise 'Column N' headers are generated. JSON arrays-of-objects always use the object keys."),
        )
        .param(
            Param::enumv("align", ["left", "right", "center"]).default("left").describe(
                "Column text alignment: left (default), right or center.",
            ),
        )
        .param(
            Param::string("null_text")
                .default("")
                .describe("Text to render for a JSON null / missing column value (default empty). Set to e.g. 'NULL' to mark database nulls."),
        )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/query-result-formatter",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Render raw query result rows (JSON/CSV/TSV) into a Markdown or ASCII table",
    skill(
        description = "Render raw query result rows into a clean, aligned table for docs and chat. data is a JSON array of row objects (columns = the object keys, unioned across rows; missing keys become null_text), a JSON array of arrays, a single JSON row object, CSV, TSV, or a psql/MySQL/SQLite CLI result table (pipe columns with +---+/---+--- rules and a (N rows) footer). input_format=auto (default) sniffs JSON vs a SQL CLI table vs TSV vs CSV. format=markdown (default) emits a GitHub-style pipe table; format=ascii emits a box-drawing grid. header (default true) treats the first CSV/TSV/array row as the header. align is left/right/center. null_text sets how JSON nulls / missing values render (default empty). Runs locally.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "query-result-formatter", |a: Args| {
            let inf = InputFormat::parse(&a.input_format).map_err(SkillError::InvalidArgs)?;
            let fmt = Format::parse(&a.format).map_err(SkillError::InvalidArgs)?;
            let al = Align::parse(&a.align).map_err(SkillError::InvalidArgs)?;
            format_result(&a.data, inf, fmt, a.header, al, &a.null_text).map_err(SkillError::InvalidArgs)
        }) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}
