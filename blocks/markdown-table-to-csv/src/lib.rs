//! gizza-ai/markdown-table-to-csv — parse a Markdown (or pipe-delimited) table
//! back into clean CSV. Thin wrapper; the chat schema is single-sourced from
//! descriptor() (which also drives the CLI); handle() delegates to run_skill.
//! Pure → runs on all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_markdown_table_to_csv_core::{to_csv_ext, Quote};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    markdown: String,
    #[serde(default)]
    delimiter: String,
    #[serde(default = "default_true")]
    header: bool,
    #[serde(default = "default_quote")]
    quote: String,
    #[serde(default = "default_newline")]
    newline: String,
    #[serde(default)]
    bom: bool,
}
fn default_true() -> bool {
    true
}
fn default_quote() -> String {
    "minimal".to_string()
}
fn default_newline() -> String {
    "lf".to_string()
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("markdown")
                .required()
                .describe("The Markdown or pipe-delimited table text. Outer pipes are optional; the alignment/separator row and cell padding are stripped, and `\\|` is treated as a literal pipe inside a cell."),
        )
        .param(
            Param::string("delimiter")
                .default(",")
                .describe("Output field separator: a single char or 'comma'/'tab'/'semicolon'/'pipe'/'space'. Use 'tab' for TSV. Default ','."),
        )
        .param(
            Param::boolean("header")
                .default(true)
                .describe("Whether the table's first row is a header to keep (default true). Set false to drop the header row and output only the data rows."),
        )
        .param(
            Param::enumv("quote", ["minimal", "all"]).default("minimal").describe(
                "CSV quoting: 'minimal' (default) quotes a field only when it contains the delimiter, a quote, or a newline; 'all' wraps every field in double quotes.",
            ),
        )
        .param(
            Param::enumv("newline", ["lf", "crlf"]).default("lf").describe(
                "Output line ending between rows: 'lf' (default, `\\n`, Unix/macOS) or 'crlf' (`\\r\\n`, Windows/Excel).",
            ),
        )
        .param(
            Param::boolean("bom")
                .default(false)
                .describe("Prepend a UTF-8 byte-order mark (BOM) so Excel opens the CSV as UTF-8. Default false."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct MarkdownTableToCsv;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/markdown-table-to-csv",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Convert a Markdown or pipe-delimited table into clean CSV",
    skill(
        description = "Parse a Markdown (or pipe-delimited) table back into clean CSV. Strips the alignment/separator row (`---`, `:--`, `--:`, `:-:`) and cell padding, honors optional outer pipes, and unescapes `\\|` into a literal pipe inside a cell. delimiter sets the output field separator (single char or comma/tab/semicolon/pipe/space; 'tab' gives TSV; default ','). header=true (default) keeps the first row; header=false drops it and emits data rows only. quote=minimal (default) quotes only fields that need it (RFC-4180); quote=all wraps every field. newline=lf (default) or crlf sets the row line ending; bom=true prepends a UTF-8 BOM for Excel. Ragged rows are padded to the widest row. Runs locally.",
        parameters = schema_json()
    ),
)]
impl MarkdownTableToCsv {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "markdown-table-to-csv", |a: Args| {
            let q = Quote::parse(&a.quote).map_err(SkillError::InvalidArgs)?;
            let delim = if a.delimiter.is_empty() { ",".to_string() } else { a.delimiter };
            let crlf = match a.newline.trim().to_ascii_lowercase().as_str() {
                "" | "lf" | "\n" => false,
                "crlf" | "\r\n" => true,
                other => {
                    return Err(SkillError::InvalidArgs(format!(
                        "unknown newline '{other}' (use lf or crlf)"
                    )))
                }
            };
            to_csv_ext(&a.markdown, &delim, a.header, q, crlf, a.bom)
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
                    "markdown": { "type": "string", "description": "The Markdown or pipe-delimited table text. Outer pipes are optional; the alignment/separator row and cell padding are stripped, and `\\|` is treated as a literal pipe inside a cell." },
                    "delimiter": { "type": "string", "default": ",", "description": "Output field separator: a single char or 'comma'/'tab'/'semicolon'/'pipe'/'space'. Use 'tab' for TSV. Default ','." },
                    "header": { "type": "boolean", "default": true, "description": "Whether the table's first row is a header to keep (default true). Set false to drop the header row and output only the data rows." },
                    "quote": { "type": "string", "enum": ["minimal", "all"], "default": "minimal", "description": "CSV quoting: 'minimal' (default) quotes a field only when it contains the delimiter, a quote, or a newline; 'all' wraps every field in double quotes." },
                    "newline": { "type": "string", "enum": ["lf", "crlf"], "default": "lf", "description": "Output line ending between rows: 'lf' (default, `\\n`, Unix/macOS) or 'crlf' (`\\r\\n`, Windows/Excel)." },
                    "bom": { "type": "boolean", "default": false, "description": "Prepend a UTF-8 byte-order mark (BOM) so Excel opens the CSV as UTF-8. Default false." }
                },
                "required": ["markdown"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
