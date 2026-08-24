//! gizza-ai/csv-numeric-column-extractor — chat skill block on the shared tool abstraction.
//!
//! Parses CSV/TSV text, decides which columns are fully numeric, and returns those
//! columns as typed arrays with their headers. The chat schema is single-sourced
//! from `descriptor()` (which also drives the CLI); `handle()` delegates to
//! `block_utils::run_skill`.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_csv_numeric_column_extractor_core::DEFAULT_NULL_TOKENS;
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    data: String,
    #[serde(default)]
    delimiter: String,
    #[serde(default)]
    header: String,
    #[serde(default)]
    output: String,
    #[serde(default)]
    null_tokens: String,
    #[serde(default = "default_true")]
    allow_blanks: bool,
    #[serde(default = "default_ratio")]
    min_numeric_ratio: f64,
    #[serde(default = "default_true")]
    normalize: bool,
}

fn default_true() -> bool {
    true
}

fn default_ratio() -> f64 {
    1.0
}

/// Single source for the chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("data")
                .required()
                .describe("The CSV/TSV text to scan (paste the file contents, including the header row if there is one), e.g. 'id,name,score\\n1,Alice,9.5'."),
        )
        .param(
            Param::enumv("delimiter", ["auto", "comma", "tab", "semicolon", "pipe"])
                .default("auto")
                .describe("Field delimiter. 'auto' (default) sniffs it by parsing the data with comma, tab, semicolon and pipe and keeping the one with the most consistent column count; otherwise force comma, tab, semicolon or pipe."),
        )
        .param(
            Param::enumv("header", ["auto", "present", "absent"])
                .default("auto")
                .describe("Whether the first row holds column names. 'auto' (default) treats it as a header unless one of its cells is a number; 'present' forces it; 'absent' treats every row as data and names the columns column_1, column_2, ..."),
        )
        .param(
            Param::enumv("output", ["columns", "records", "csv", "names"])
                .default("columns")
                .describe("Result shape. 'columns' (default) = JSON with one typed array per numeric column plus a 'skipped' list explaining every rejected column; 'records' = JSON row objects holding only the numeric fields; 'csv' = the numeric columns as CSV; 'names' = just the numeric column names, one per line."),
        )
        .param(
            Param::string("null_tokens")
                .default(DEFAULT_NULL_TOKENS)
                .describe("Comma-separated tokens (in addition to the empty cell) treated as missing, e.g. 'NA,N/A,NULL'. Matching is exact and case-sensitive; missing cells become null in the output."),
        )
        .param(
            Param::boolean("allow_blanks")
                .default(true)
                .describe("Keep a column that is numeric apart from blank/null cells (default true; the gaps become null). Set false to require every cell to hold a value."),
        )
        .param(
            Param::number("min_numeric_ratio")
                .default(1.0)
                .min(0.1)
                .max(1.0)
                .describe("Fraction of a column's non-missing cells that must parse as numbers for it to count as numeric. 1.0 (default) = every value; 0.9 tolerates a stray label like 'n/a-ish', which is then emitted as null."),
        )
        .param(
            Param::boolean("normalize")
                .default(true)
                .describe("Accept accounting-formatted numbers: thousands separators ('1,234.50'), currency symbols ($ € £ ¥ ₹), trailing percent ('45%'), parentheses negatives ('(500)' = -500) and trailing minus ('250-'). Default true; set false to require plain numbers."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/csv-numeric-column-extractor",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Find a CSV's numeric columns and return them as typed arrays with headers.",
    skill(
        description = "Parse CSV/TSV text, detect which columns are numeric, and return those columns as typed arrays with their headers (plus a 'skipped' list saying why every other column was rejected). delimiter='auto' (default) sniffs comma/tab/semicolon/pipe; header='auto' (default) detects the header row; output='columns' (default) emits JSON typed arrays, 'records' JSON row objects, 'csv' the numeric columns as CSV, 'names' just the column names; null_tokens lists strings treated as missing (default 'NA,N/A,NULL,null,None,nan'); allow_blanks=true (default) keeps columns with gaps; min_numeric_ratio=1.0 (default) requires every value to parse; normalize=true (default) accepts '1,234.50', '$99', '45%', '(500)' and '250-'. Zero-padded codes like '007' stay non-numeric. Input is capped at 1 MB.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "csv-numeric-column-extractor", |a: Args| {
            let null_tokens = if a.null_tokens.trim().is_empty() {
                DEFAULT_NULL_TOKENS
            } else {
                a.null_tokens.as_str()
            };
            gizza_ai_csv_numeric_column_extractor_core::extract(
                &a.data,
                &a.delimiter,
                &a.header,
                &a.output,
                null_tokens,
                a.allow_blanks,
                a.min_numeric_ratio,
                a.normalize,
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

    /// Drift guard: the descriptor-derived chat schema must match this authored
    /// schema, so any future change to the LLM-facing API is intentional and
    /// reviewed.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "data": { "type": "string", "description": "The CSV/TSV text to scan (paste the file contents, including the header row if there is one), e.g. 'id,name,score\\n1,Alice,9.5'." },
                    "delimiter": { "type": "string", "enum": ["auto", "comma", "tab", "semicolon", "pipe"], "default": "auto", "description": "Field delimiter. 'auto' (default) sniffs it by parsing the data with comma, tab, semicolon and pipe and keeping the one with the most consistent column count; otherwise force comma, tab, semicolon or pipe." },
                    "header": { "type": "string", "enum": ["auto", "present", "absent"], "default": "auto", "description": "Whether the first row holds column names. 'auto' (default) treats it as a header unless one of its cells is a number; 'present' forces it; 'absent' treats every row as data and names the columns column_1, column_2, ..." },
                    "output": { "type": "string", "enum": ["columns", "records", "csv", "names"], "default": "columns", "description": "Result shape. 'columns' (default) = JSON with one typed array per numeric column plus a 'skipped' list explaining every rejected column; 'records' = JSON row objects holding only the numeric fields; 'csv' = the numeric columns as CSV; 'names' = just the numeric column names, one per line." },
                    "null_tokens": { "type": "string", "default": "NA,N/A,NULL,null,None,nan", "description": "Comma-separated tokens (in addition to the empty cell) treated as missing, e.g. 'NA,N/A,NULL'. Matching is exact and case-sensitive; missing cells become null in the output." },
                    "allow_blanks": { "type": "boolean", "default": true, "description": "Keep a column that is numeric apart from blank/null cells (default true; the gaps become null). Set false to require every cell to hold a value." },
                    "min_numeric_ratio": { "type": "number", "default": 1.0, "minimum": 0.1, "maximum": 1, "description": "Fraction of a column's non-missing cells that must parse as numbers for it to count as numeric. 1.0 (default) = every value; 0.9 tolerates a stray label like 'n/a-ish', which is then emitted as null." },
                    "normalize": { "type": "boolean", "default": true, "description": "Accept accounting-formatted numbers: thousands separators ('1,234.50'), currency symbols ($ € £ ¥ ₹), trailing percent ('45%'), parentheses negatives ('(500)' = -500) and trailing minus ('250-'). Default true; set false to require plain numbers." }
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
