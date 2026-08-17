//! gizza-ai/csv-quote-normalizer — chat skill block on the shared tool abstraction.
//! Reads a CSV with a deliberately tolerant parser (backslash-escaped quotes,
//! single-quoted or curly-quoted fields, padding around quotes, stray quotes, an
//! unclosed quote at EOF) and re-emits it with ONE consistent dialect: a chosen
//! quoting policy, escape convention, quote character, delimiter and line ending.
//! The chat schema is single-sourced from `descriptor()` (which also drives the
//! CLI); `handle()` delegates to `block_utils::run_skill`. Pure compute — nothing
//! is uploaded.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    input: String,
    #[serde(default)]
    delimiter: String,
    #[serde(default)]
    output_delimiter: String,
    #[serde(default)]
    input_quote: String,
    #[serde(default)]
    quote_style: String,
    #[serde(default)]
    output_quote: String,
    #[serde(default)]
    escape: String,
    #[serde(default = "default_true")]
    backslash_escapes: bool,
    #[serde(default = "default_true")]
    smart_quotes: bool,
    #[serde(default)]
    line_ending: String,
    #[serde(default)]
    output: String,
}

fn default_true() -> bool {
    true
}

/// Single source for the chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("input")
                .required()
                .describe("The CSV/delimited text to re-quote. The parser is deliberately tolerant, so the files a strict reader rejects are read rather than refused: backslash-escaped quotes, single-quoted or curly-quoted fields, padding before an opening quote, a stray quote inside a value, text after a closing quote, and an unclosed quote at end of input. Max 5,000,000 bytes."),
        )
        .param(
            Param::string("delimiter")
                .default("auto")
                .describe("Field separator of the INPUT: 'auto' (default) sniffs the most frequent candidate (comma, semicolon, tab, pipe) outside quotes on the first logical line, or give a single character or a name ('comma', 'tab', 'semicolon', 'pipe', 'space')."),
        )
        .param(
            Param::string("output_delimiter")
                .default("same")
                .describe("Field separator of the OUTPUT: 'same' (default) reuses the input's separator, or give a single character or a name ('comma', 'tab', 'semicolon', 'pipe', 'space') to switch the file to another dialect on the way out."),
        )
        .param(
            Param::enumv("input_quote", ["auto", "double", "single", "none"])
                .default("auto")
                .describe("Quote character to read in the INPUT: 'auto' (default) picks double when any straight or curly double quote appears, and single only when a ' actually opens a field, so an apostrophe in ordinary prose is never mistaken for a quote character; 'double'; 'single'; or 'none' to treat every quote character as literal content."),
        )
        .param(
            Param::enumv("quote_style", ["minimal", "always", "non_numeric", "never"])
                .default("minimal")
                .describe("Which OUTPUT fields get quoted: 'minimal' (default) quotes only a value that contains the delimiter, the quote character or a line break; 'always' quotes every field, which keeps a diff stable and stops a spreadsheet retyping ids; 'non_numeric' quotes everything that is not a plain decimal number (Python's csv.QUOTE_NONNUMERIC); 'never' quotes nothing and then needs escape = 'backslash' to represent a value containing a delimiter, a quote or a newline. Under the first three a value that MUST be quoted to stay readable is always quoted, whatever the policy asks for."),
        )
        .param(
            Param::enumv("output_quote", ["double", "single"])
                .default("double")
                .describe("Quote character of the OUTPUT: 'double' (default, what RFC 4180 and every spreadsheet expect) or 'single'."),
        )
        .param(
            Param::enumv("escape", ["doubled", "backslash"])
                .default("doubled")
                .describe("How a quote inside a quoted OUTPUT field is escaped: 'doubled' (default) writes \"\" per RFC 4180, which Excel, Sheets, pandas and every strict reader understand; 'backslash' writes \\\" (and \\\\ for a literal backslash) for MySQL LOAD DATA, older Postgres COPY and JavaScript-flavoured readers."),
        )
        .param(
            Param::boolean("backslash_escapes")
                .default(true)
                .describe("When true (default), \\\" in the INPUT is read as an escaped quote and \\\\ as a literal backslash — the convention MySQL, MongoDB and many hand-rolled exporters emit, and the one that makes a strict RFC 4180 reader mis-split the row. Turn it off when backslash is ordinary content (Windows paths, regexes). A trailing \\ immediately before a field-closing quote stays literal either way, so \"C:\\dir\\\" is not read as an unterminated field."),
        )
        .param(
            Param::boolean("smart_quotes")
                .default(true)
                .describe("When true (default), curly quotes (\u{201C} \u{201D} \u{2018} \u{2019}) are read as field quotes — what a CSV that has been through Word, Google Docs or any autocorrecting editor actually contains. Turn it off to keep curly quotes as ordinary characters inside the value."),
        )
        .param(
            Param::enumv("line_ending", ["lf", "crlf"])
                .default("lf")
                .describe("Line ending of the OUTPUT: 'lf' (default, \\n) or 'crlf' (\\r\\n, what RFC 4180 and older Windows/Excel tooling expect). A newline embedded inside a quoted field takes the same ending, so the whole file is consistent."),
        )
        .param(
            Param::enumv("output", ["csv", "report"])
                .default("csv")
                .describe("What to return: 'csv' (default) is the rewritten file; 'report' is a plain-text audit — the detected input dialect, the chosen output dialect, row/field/quoted counts, a ragged-row warning, and every repair the tolerant parser had to make with the line numbers it made them on."),
        )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/csv-quote-normalizer",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Re-emit a CSV with one consistent quoting, escaping, quote-character, delimiter and line-ending dialect.",
    skill(
        description = "Rewrite a CSV so every field uses ONE quoting dialect. The input is read with a deliberately tolerant parser, so the files that make a strict reader fail or silently mis-split a row are handled instead of rejected: backslash-escaped quotes (\\\" as MySQL and many exporters write them), single-quoted fields, curly \u{201C}smart\u{201D} quotes left by Word or Google Docs, padding before an opening quote, a stray un-escaped quote inside a quoted value, text running on after a closing quote, an unclosed quote at end of input, a BOM, and blank lines. The output is strict and consistent: quote_style picks which fields are quoted ('minimal' (default), 'always', 'non_numeric', 'never'), escape picks the escaping ('doubled' for RFC 4180 \"\", 'backslash' for \\\"), output_quote picks double or single, output_delimiter can switch the separator, and line_ending picks LF or CRLF — embedded newlines included. delimiter and input_quote default to 'auto' and sniff the input's dialect; backslash_escapes and smart_quotes (both on) control how tolerant the read is. Set output = 'report' to get an audit of the detected dialect, the row/field/quoted counts and every repair with its line number instead of the file. Only quoting, escaping, the delimiter and line endings change — no cell value is trimmed, retyped, padded or dropped, and ragged rows keep their length. Runs entirely in the sandbox; nothing is uploaded.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "csv-quote-normalizer", |a: Args| {
            gizza_ai_csv_quote_normalizer_core::normalize(
                &a.input,
                &a.delimiter,
                &a.output_delimiter,
                &a.input_quote,
                &a.quote_style,
                &a.output_quote,
                &a.escape,
                a.backslash_escapes,
                a.smart_quotes,
                &a.line_ending,
                &a.output,
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
    /// reviewed. Authored 2026-08-17 for the initial csv-quote-normalizer release.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "input": { "type": "string", "description": "The CSV/delimited text to re-quote. The parser is deliberately tolerant, so the files a strict reader rejects are read rather than refused: backslash-escaped quotes, single-quoted or curly-quoted fields, padding before an opening quote, a stray quote inside a value, text after a closing quote, and an unclosed quote at end of input. Max 5,000,000 bytes." },
                    "delimiter": { "type": "string", "default": "auto", "description": "Field separator of the INPUT: 'auto' (default) sniffs the most frequent candidate (comma, semicolon, tab, pipe) outside quotes on the first logical line, or give a single character or a name ('comma', 'tab', 'semicolon', 'pipe', 'space')." },
                    "output_delimiter": { "type": "string", "default": "same", "description": "Field separator of the OUTPUT: 'same' (default) reuses the input's separator, or give a single character or a name ('comma', 'tab', 'semicolon', 'pipe', 'space') to switch the file to another dialect on the way out." },
                    "input_quote": { "type": "string", "enum": ["auto", "double", "single", "none"], "default": "auto", "description": "Quote character to read in the INPUT: 'auto' (default) picks double when any straight or curly double quote appears, and single only when a ' actually opens a field, so an apostrophe in ordinary prose is never mistaken for a quote character; 'double'; 'single'; or 'none' to treat every quote character as literal content." },
                    "quote_style": { "type": "string", "enum": ["minimal", "always", "non_numeric", "never"], "default": "minimal", "description": "Which OUTPUT fields get quoted: 'minimal' (default) quotes only a value that contains the delimiter, the quote character or a line break; 'always' quotes every field, which keeps a diff stable and stops a spreadsheet retyping ids; 'non_numeric' quotes everything that is not a plain decimal number (Python's csv.QUOTE_NONNUMERIC); 'never' quotes nothing and then needs escape = 'backslash' to represent a value containing a delimiter, a quote or a newline. Under the first three a value that MUST be quoted to stay readable is always quoted, whatever the policy asks for." },
                    "output_quote": { "type": "string", "enum": ["double", "single"], "default": "double", "description": "Quote character of the OUTPUT: 'double' (default, what RFC 4180 and every spreadsheet expect) or 'single'." },
                    "escape": { "type": "string", "enum": ["doubled", "backslash"], "default": "doubled", "description": "How a quote inside a quoted OUTPUT field is escaped: 'doubled' (default) writes \"\" per RFC 4180, which Excel, Sheets, pandas and every strict reader understand; 'backslash' writes \\\" (and \\\\ for a literal backslash) for MySQL LOAD DATA, older Postgres COPY and JavaScript-flavoured readers." },
                    "backslash_escapes": { "type": "boolean", "default": true, "description": "When true (default), \\\" in the INPUT is read as an escaped quote and \\\\ as a literal backslash — the convention MySQL, MongoDB and many hand-rolled exporters emit, and the one that makes a strict RFC 4180 reader mis-split the row. Turn it off when backslash is ordinary content (Windows paths, regexes). A trailing \\ immediately before a field-closing quote stays literal either way, so \"C:\\dir\\\" is not read as an unterminated field." },
                    "smart_quotes": { "type": "boolean", "default": true, "description": "When true (default), curly quotes (\u201C \u201D \u2018 \u2019) are read as field quotes — what a CSV that has been through Word, Google Docs or any autocorrecting editor actually contains. Turn it off to keep curly quotes as ordinary characters inside the value." },
                    "line_ending": { "type": "string", "enum": ["lf", "crlf"], "default": "lf", "description": "Line ending of the OUTPUT: 'lf' (default, \\n) or 'crlf' (\\r\\n, what RFC 4180 and older Windows/Excel tooling expect). A newline embedded inside a quoted field takes the same ending, so the whole file is consistent." },
                    "output": { "type": "string", "enum": ["csv", "report"], "default": "csv", "description": "What to return: 'csv' (default) is the rewritten file; 'report' is a plain-text audit — the detected input dialect, the chosen output dialect, row/field/quoted counts, a ragged-row warning, and every repair the tolerant parser had to make with the line numbers it made them on." }
                },
                "required": ["input"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
