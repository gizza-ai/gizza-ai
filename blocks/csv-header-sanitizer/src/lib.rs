//! gizza-ai/csv-header-sanitizer — chat skill block on the shared tool abstraction.
//! Rewrites the HEADER ROW of a CSV/delimited table into valid, consistent
//! identifiers (snake_case by default), repairs blank and digit-leading names,
//! and deduplicates collisions so two source columns can never clean to the same
//! label. The chat schema is single-sourced from `descriptor()` (which also
//! drives the CLI); `handle()` delegates to `block_utils::run_skill`. Pure
//! compute — nothing is uploaded.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    data: String,
    #[serde(default)]
    delimiter: String,
    #[serde(default)]
    style: String,
    #[serde(default = "default_true")]
    ascii: bool,
    #[serde(default)]
    leading_digit: String,
    #[serde(default)]
    max_length: u32,
    #[serde(default = "default_blank_name")]
    blank_name: String,
    #[serde(default)]
    dedupe: String,
    #[serde(default)]
    output: String,
}

fn default_true() -> bool {
    true
}
fn default_blank_name() -> String {
    "column".to_string()
}

/// Single source for the chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("data")
                .required()
                .describe("The CSV/delimited table to clean, as text. Row 1 is the header — it is the only row rewritten. Quoted fields with embedded separators or newlines (RFC 4180) are preserved and data rows are passed through unchanged. Max 5,000,000 bytes."),
        )
        .param(
            Param::string("delimiter")
                .default(",")
                .describe("Field separator: 'auto' to sniff it from the header line, a single character, or a name ('comma' (default), 'tab', 'semicolon', 'pipe'). The output uses the same separator as the input."),
        )
        .param(
            Param::enumv(
                "style",
                ["snake", "camel", "pascal", "kebab", "screaming_snake", "lower", "preserve"],
            )
            .default("snake")
            .describe("Target identifier casing for 'First Name': 'snake' -> first_name (default), 'camel' -> firstName, 'pascal' -> FirstName, 'kebab' -> first-name, 'screaming_snake' -> FIRST_NAME, 'lower' -> lowercase without splitting CamelCase runs (FirstName -> firstname), 'preserve' -> keep the original case and only fix the characters (First_Name)."),
        )
        .param(
            Param::boolean("ascii")
                .default(true)
                .describe("When true (default), Unicode is transliterated to ASCII before cleaning, so 'Año' becomes ano and 'Größe' becomes grosse. Turn it off to keep non-ASCII letters in the names (valid for quoted SQL identifiers and most dataframe libraries, but not for bare identifiers)."),
        )
        .param(
            Param::enumv("leading_digit", ["underscore", "col", "keep"])
                .default("underscore")
                .describe("What to do when a name would start with a digit, which an unquoted SQL identifier cannot: 'underscore' (default) prefixes an underscore ('2024 Revenue' -> _2024_revenue), 'col' prefixes the word col (col_2024_revenue), 'keep' leaves it as 2024_revenue."),
        )
        .param(
            Param::integer("max_length")
                .default(0)
                .min(0.0)
                .max(300.0)
                .describe("Truncate each name to at most this many characters, cutting any dangling separator. 0 (default) means no limit. Use 63 for PostgreSQL identifiers; 300 is BigQuery's ceiling. A deduplication suffix is always kept inside the cap — the base name gives up characters to make room."),
        )
        .param(
            Param::string("blank_name")
                .default("column")
                .describe("Base name for a header cell that is blank or nothing but punctuation; the column's 1-based position is appended, so the default gives column_2, column_3. Set it to something like 'field' or 'unnamed' to match your own convention."),
        )
        .param(
            Param::enumv("dedupe", ["suffix", "index", "allow"])
                .default("suffix")
                .describe("What to do when two headers clean to the same name: 'suffix' (default) counts up — total, total_2, total_3; 'index' names the duplicate after its own 1-based column position — total, total_3; 'allow' leaves the collision in place, which is only safe if the reader keeps duplicate columns."),
        )
        .param(
            Param::enumv("output", ["csv", "header", "mapping"])
                .default("csv")
                .describe("What to return: 'csv' (default) is the whole table with the rewritten header row; 'header' is just the cleaned header line; 'mapping' is a two-column 'original,sanitized' audit trail so you can review every rename before applying it."),
        )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/csv-header-sanitizer",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Clean CSV column names into valid, consistent snake_case identifiers, deduplicating collisions.",
    skill(
        description = "Sanitize the header row of a CSV/delimited table into valid, consistent identifiers. 'First Name' becomes first_name, 'Total ($)' becomes total, '2024 Revenue' becomes _2024_revenue, a blank header becomes column_3, and two columns that clean to the same name are deduplicated (total, total_2) so neither is silently lost in a downstream join or import. Only row 1 is rewritten: data rows pass through untouched, quoting is preserved, and the field separator round-trips unchanged. style picks the casing: snake (default), camel, pascal, kebab, screaming_snake, lower (no CamelCase splitting), or preserve (keep the original case). ascii (default on) transliterates Unicode to ASCII. leading_digit repairs names starting with a digit via 'underscore' (default), 'col', or 'keep'. max_length truncates names (0 = no limit; 63 is the PostgreSQL identifier limit) and always keeps a dedupe suffix inside the cap. blank_name is the base for empty headers. dedupe is 'suffix' (default), 'index', or 'allow'. output is 'csv' (default, the whole table), 'header' (just the cleaned header line), or 'mapping' (an original,sanitized audit trail). delimiter accepts 'auto', a single character, or comma/tab/semicolon/pipe. This renames columns only — it never touches the data values, dedupes rows, or infers types. Runs entirely in the sandbox; nothing is uploaded.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "csv-header-sanitizer", |a: Args| {
            gizza_ai_csv_header_sanitizer_core::sanitize(
                &a.data,
                &a.delimiter,
                &a.style,
                a.ascii,
                &a.leading_digit,
                a.max_length,
                &a.blank_name,
                &a.dedupe,
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
    /// reviewed. Authored 2026-08-16 for the initial csv-header-sanitizer release.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "data": { "type": "string", "description": "The CSV/delimited table to clean, as text. Row 1 is the header — it is the only row rewritten. Quoted fields with embedded separators or newlines (RFC 4180) are preserved and data rows are passed through unchanged. Max 5,000,000 bytes." },
                    "delimiter": { "type": "string", "default": ",", "description": "Field separator: 'auto' to sniff it from the header line, a single character, or a name ('comma' (default), 'tab', 'semicolon', 'pipe'). The output uses the same separator as the input." },
                    "style": { "type": "string", "enum": ["snake", "camel", "pascal", "kebab", "screaming_snake", "lower", "preserve"], "default": "snake", "description": "Target identifier casing for 'First Name': 'snake' -> first_name (default), 'camel' -> firstName, 'pascal' -> FirstName, 'kebab' -> first-name, 'screaming_snake' -> FIRST_NAME, 'lower' -> lowercase without splitting CamelCase runs (FirstName -> firstname), 'preserve' -> keep the original case and only fix the characters (First_Name)." },
                    "ascii": { "type": "boolean", "default": true, "description": "When true (default), Unicode is transliterated to ASCII before cleaning, so 'Año' becomes ano and 'Größe' becomes grosse. Turn it off to keep non-ASCII letters in the names (valid for quoted SQL identifiers and most dataframe libraries, but not for bare identifiers)." },
                    "leading_digit": { "type": "string", "enum": ["underscore", "col", "keep"], "default": "underscore", "description": "What to do when a name would start with a digit, which an unquoted SQL identifier cannot: 'underscore' (default) prefixes an underscore ('2024 Revenue' -> _2024_revenue), 'col' prefixes the word col (col_2024_revenue), 'keep' leaves it as 2024_revenue." },
                    "max_length": { "type": "integer", "minimum": 0, "maximum": 300, "default": 0, "description": "Truncate each name to at most this many characters, cutting any dangling separator. 0 (default) means no limit. Use 63 for PostgreSQL identifiers; 300 is BigQuery's ceiling. A deduplication suffix is always kept inside the cap — the base name gives up characters to make room." },
                    "blank_name": { "type": "string", "default": "column", "description": "Base name for a header cell that is blank or nothing but punctuation; the column's 1-based position is appended, so the default gives column_2, column_3. Set it to something like 'field' or 'unnamed' to match your own convention." },
                    "dedupe": { "type": "string", "enum": ["suffix", "index", "allow"], "default": "suffix", "description": "What to do when two headers clean to the same name: 'suffix' (default) counts up — total, total_2, total_3; 'index' names the duplicate after its own 1-based column position — total, total_3; 'allow' leaves the collision in place, which is only safe if the reader keeps duplicate columns." },
                    "output": { "type": "string", "enum": ["csv", "header", "mapping"], "default": "csv", "description": "What to return: 'csv' (default) is the whole table with the rewritten header row; 'header' is just the cleaned header line; 'mapping' is a two-column 'original,sanitized' audit trail so you can review every rename before applying it." }
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
