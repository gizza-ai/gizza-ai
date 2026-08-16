//! gizza-ai/markdown-table-extractor — find every GitHub-flavored Markdown table
//! in a document and export the selected ones as CSV, JSON, JSON Lines, or an
//! inventory listing. Thin wrapper; the chat schema is single-sourced from
//! descriptor() (which also drives the CLI); handle() delegates to run_skill.
//! Pure → runs on all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_markdown_table_extractor_core::{extract, parse_format, Options, Quote};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    markdown: String,
    #[serde(default)]
    format: String,
    #[serde(default)]
    table: String,
    #[serde(default = "default_true")]
    header: bool,
    #[serde(default)]
    delimiter: String,
    #[serde(default)]
    quote: String,
    #[serde(default)]
    newline: String,
    #[serde(default = "default_true")]
    trim: bool,
    #[serde(default)]
    strip_formatting: bool,
    #[serde(default = "default_indent")]
    json_indent: f64,
    #[serde(default = "default_true")]
    labels: bool,
}
fn default_true() -> bool {
    true
}
fn default_indent() -> f64 {
    2.0
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("markdown")
                .required()
                .describe("The Markdown document to scan. It can be a whole README or page of prose — every GitHub-flavored pipe table in it is found (a header row plus a `|---|` separator row directly under it). Pipe lines inside ``` or ~~~ code fences are ignored. Max 1000000 bytes."),
        )
        .param(
            Param::enumv("format", ["csv", "json", "jsonl", "list"])
                .default("csv")
                .describe("Output format. 'csv' (default) emits one delimited block per table; 'json' emits an array of rows for a single table, or an array of {index, heading, line, columns, rows} envelopes for several; 'jsonl' emits one JSON value per data row (rows are wrapped as {\"table\":n,\"row\":…} when several tables are exported); 'list' emits an inventory of the tables found (index, heading, source line, columns, alignments, row count) without any cell data."),
        )
        .param(
            Param::string("table")
                .default("all")
                .describe("Which tables to export: 'all' (default, in document order), a single 0-based index like '2', or a comma-separated list/range like '0,2-3'. Use format='list' first to see what is in the document. An index past the last table is an error naming the valid range."),
        )
        .param(
            Param::boolean("header")
                .default(true)
                .describe("Treat each table's first row as a header (default true). CSV keeps it as the first line; JSON/JSONL key each row object by it. Set false to drop it: CSV emits data rows only and JSON/JSONL emit arrays of values instead of objects."),
        )
        .param(
            Param::string("delimiter")
                .default(",")
                .describe("CSV field separator: a single character or 'comma'/'tab'/'semicolon'/'pipe'/'space'. Use 'tab' for TSV. Default ','. Ignored for json, jsonl and list."),
        )
        .param(
            Param::enumv("quote", ["minimal", "all"])
                .default("minimal")
                .describe("CSV quoting: 'minimal' (default) quotes a field only when it contains the delimiter, a double quote, or a newline; 'all' wraps every field in double quotes."),
        )
        .param(
            Param::enumv("newline", ["lf", "crlf"])
                .default("lf")
                .describe("Line ending between output rows: 'lf' (default, `\\n`, Unix/macOS) or 'crlf' (`\\r\\n`, Windows/Excel)."),
        )
        .param(
            Param::boolean("trim")
                .default(true)
                .describe("Trim the whitespace padding Markdown authors use to align columns (default true). Set false to keep each cell exactly as written, spaces included."),
        )
        .param(
            Param::boolean("strip_formatting")
                .default(false)
                .describe("Render inline Markdown inside cells as plain text: `**bold**` → bold, `` `code` `` → code, `[text](url)` → text, `<br>` → a space, and `\\|`-style escapes resolved. Default false, which keeps every cell exactly as written."),
        )
        .param(
            Param::integer("json_indent")
                .default(2)
                .min(0.0)
                .max(8.0)
                .describe("Indent width in spaces for json and list output; 0 minifies to a single line. Default 2. Ignored for csv (and for jsonl, where each line is always compact)."),
        )
        .param(
            Param::boolean("labels")
                .default(true)
                .describe("When several tables are exported as CSV, prefix each block with a `# Table n: heading` comment line (default true). Set false for plain blocks separated only by a blank line. Has no effect on a single table or on the other formats."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct MarkdownTableExtractor;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/markdown-table-extractor",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Find every Markdown table in a document and export it as CSV or JSON",
    skill(
        description = "Find every GitHub-flavored Markdown table in a document and export the ones you pick as CSV, JSON, JSON Lines, or an inventory listing. A table is a pipe-bearing header row followed by a `|---|` separator row with the same cell count; pipe lines inside ``` or ~~~ code fences are ignored, and each table records the nearest preceding heading and its source line. table='all' (default) exports every table in document order, or pass an index ('2') or a list/range ('0,2-3'); format='list' first shows what the document contains. format='csv' (default) writes one delimited block per table, blank-line separated and prefixed with a `# Table n` comment when several are exported (labels=false turns that off); 'json' gives an array of row objects for one table or of table envelopes for several; 'jsonl' gives one JSON value per data row. header=true (default) keys rows by the header row; false drops it and emits arrays. delimiter/quote/newline control the CSV (single char or comma/tab/semicolon/pipe/space; minimal or all; lf or crlf), trim strips cell padding, strip_formatting renders bold/code/links/<br> as plain text, and json_indent sets the JSON indent (0 minifies). Rows shorter than the header are padded and extra cells are dropped, exactly as Markdown renders them. Input is capped at 1000000 bytes. Runs locally.",
        parameters = schema_json()
    ),
)]
impl MarkdownTableExtractor {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "markdown-table-extractor", |a: Args| {
            let format = parse_format(&a.format).map_err(SkillError::InvalidArgs)?;
            let quote = Quote::parse(&a.quote).map_err(SkillError::InvalidArgs)?;
            let crlf = match a.newline.trim().to_ascii_lowercase().as_str() {
                "" | "lf" | "\n" => false,
                "crlf" | "\r\n" => true,
                other => {
                    return Err(SkillError::InvalidArgs(format!(
                        "unknown newline '{other}' (expected lf or crlf)"
                    )))
                }
            };
            if !(0.0..=8.0).contains(&a.json_indent) {
                return Err(SkillError::InvalidArgs(format!(
                    "json_indent must be between 0 and 8, got {}",
                    a.json_indent
                )));
            }
            let opts = Options {
                format,
                table: if a.table.trim().is_empty() { "all".into() } else { a.table },
                header: a.header,
                delimiter: if a.delimiter.is_empty() { ",".into() } else { a.delimiter },
                quote,
                crlf,
                trim: a.trim,
                strip_formatting: a.strip_formatting,
                json_indent: a.json_indent as usize,
                labels: a.labels,
            };
            extract(&a.markdown, &opts).map_err(SkillError::InvalidArgs)
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
                    "markdown": { "type": "string", "description": "The Markdown document to scan. It can be a whole README or page of prose — every GitHub-flavored pipe table in it is found (a header row plus a `|---|` separator row directly under it). Pipe lines inside ``` or ~~~ code fences are ignored. Max 1000000 bytes." },
                    "format": { "type": "string", "enum": ["csv", "json", "jsonl", "list"], "default": "csv", "description": "Output format. 'csv' (default) emits one delimited block per table; 'json' emits an array of rows for a single table, or an array of {index, heading, line, columns, rows} envelopes for several; 'jsonl' emits one JSON value per data row (rows are wrapped as {\"table\":n,\"row\":…} when several tables are exported); 'list' emits an inventory of the tables found (index, heading, source line, columns, alignments, row count) without any cell data." },
                    "table": { "type": "string", "default": "all", "description": "Which tables to export: 'all' (default, in document order), a single 0-based index like '2', or a comma-separated list/range like '0,2-3'. Use format='list' first to see what is in the document. An index past the last table is an error naming the valid range." },
                    "header": { "type": "boolean", "default": true, "description": "Treat each table's first row as a header (default true). CSV keeps it as the first line; JSON/JSONL key each row object by it. Set false to drop it: CSV emits data rows only and JSON/JSONL emit arrays of values instead of objects." },
                    "delimiter": { "type": "string", "default": ",", "description": "CSV field separator: a single character or 'comma'/'tab'/'semicolon'/'pipe'/'space'. Use 'tab' for TSV. Default ','. Ignored for json, jsonl and list." },
                    "quote": { "type": "string", "enum": ["minimal", "all"], "default": "minimal", "description": "CSV quoting: 'minimal' (default) quotes a field only when it contains the delimiter, a double quote, or a newline; 'all' wraps every field in double quotes." },
                    "newline": { "type": "string", "enum": ["lf", "crlf"], "default": "lf", "description": "Line ending between output rows: 'lf' (default, `\\n`, Unix/macOS) or 'crlf' (`\\r\\n`, Windows/Excel)." },
                    "trim": { "type": "boolean", "default": true, "description": "Trim the whitespace padding Markdown authors use to align columns (default true). Set false to keep each cell exactly as written, spaces included." },
                    "strip_formatting": { "type": "boolean", "default": false, "description": "Render inline Markdown inside cells as plain text: `**bold**` → bold, `` `code` `` → code, `[text](url)` → text, `<br>` → a space, and `\\|`-style escapes resolved. Default false, which keeps every cell exactly as written." },
                    "json_indent": { "type": "integer", "default": 2, "minimum": 0, "maximum": 8, "description": "Indent width in spaces for json and list output; 0 minifies to a single line. Default 2. Ignored for csv (and for jsonl, where each line is always compact)." },
                    "labels": { "type": "boolean", "default": true, "description": "When several tables are exported as CSV, prefix each block with a `# Table n: heading` comment line (default true). Set false for plain blocks separated only by a blank line. Has no effect on a single table or on the other formats." }
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
