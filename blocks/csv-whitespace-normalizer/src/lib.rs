//! gizza-ai/csv-whitespace-normalizer — chat skill block on the shared tool
//! abstraction. Trims cell edges AND collapses/removes the whitespace runs *inside*
//! each cell of a CSV/delimited table, inside a real RFC 4180 parse, with the full
//! Unicode `White_Space` set (NBSP, narrow NBSP, ideographic space, …) recognised by
//! default. The chat schema is single-sourced from `descriptor()` (which also drives
//! the CLI); `handle()` delegates to `block_utils::run_skill`. Pure compute —
//! nothing is uploaded.
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
    trim: String,
    #[serde(default)]
    internal: String,
    #[serde(default)]
    whitespace: String,
    #[serde(default)]
    columns: String,
    #[serde(default = "default_true")]
    header: bool,
    #[serde(default = "default_true")]
    normalize_header: bool,
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
                .describe("The CSV/delimited table to normalize, as text. Quoted fields with embedded separators or newlines (RFC 4180) are preserved. Rows may be ragged — a short row stays short. Max 5,000,000 bytes."),
        )
        .param(
            Param::string("delimiter")
                .default(",")
                .describe("Field separator: 'auto' to sniff it from the first line, a single character, or a name ('comma' (default), 'tab', 'semicolon', 'pipe'). The output uses the same separator as the input."),
        )
        .param(
            Param::enumv("trim", ["both", "leading", "trailing", "none"])
                .default("both")
                .describe("Which end(s) of a cell lose their whitespace: 'both' (default), 'leading' (start only), 'trailing' (end only), or 'none' to leave both edges alone and rewrite only the interior. An edge run that survives is copied verbatim."),
        )
        .param(
            Param::enumv("internal", ["collapse", "remove", "keep"])
                .default("collapse")
                .describe("What happens to whitespace BETWEEN the first and last non-whitespace character of a cell: 'collapse' (default) turns every run into one plain space, so 'Ada   Lovelace' becomes 'Ada Lovelace'; 'remove' deletes it, so 'AB 12 CD' becomes 'AB12CD' (for SKUs, IBANs, part numbers); 'keep' preserves interior spacing byte-for-byte, making this a pure trim."),
        )
        .param(
            Param::enumv("whitespace", ["unicode", "ascii"])
                .default("unicode")
                .describe("Which characters count as whitespace: 'unicode' (default) is the full Unicode White_Space set, so a non-breaking space (U+00A0), narrow NBSP (U+202F) or ideographic space (U+3000) pasted out of a spreadsheet is normalized too; 'ascii' restricts it to space, tab, newline, carriage return and form feed, leaving NBSP and friends as content. Zero-width characters (U+200B, U+FEFF) are not whitespace under either setting."),
        )
        .param(
            Param::string("columns")
                .default("")
                .describe("Restrict the pass to specific columns: a comma-separated list of column names (needs header = true), 1-based positions, and inclusive ranges, e.g. 'name,city' or '1,3-5'. Default (empty) normalizes every column. A token that parses as a number or a range is read as a position, so a header literally named '3' must be selected by its position."),
        )
        .param(
            Param::boolean("header")
                .default(true)
                .describe("When true (default), the first row is a header: it supplies the names used by the columns option. Turn it off for a headerless table, where row 1 is data like any other."),
        )
        .param(
            Param::boolean("normalize_header")
                .default(true)
                .describe("When true (default), header cells get the same whitespace pass as the data, which is what makes ' first name ' load as 'first name'. Turn it off to copy the header row through byte-for-byte. Has no effect when header is false."),
        )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/csv-whitespace-normalizer",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Trim CSV cells and collapse or remove the whitespace runs inside them, including non-breaking spaces.",
    skill(
        description = "Normalize the whitespace inside every cell of a CSV/TSV table. Unlike a plain trim, this also fixes the INTERIOR of a value: 'Ada   Lovelace' becomes 'Ada Lovelace', and with internal = 'remove', 'AB 12 CD' becomes 'AB12CD'. Padding is what silently breaks joins, VLOOKUPs, GROUP BYs and duplicate checks, because ' Berlin' and 'Berlin' are different keys. The pass runs inside a real RFC 4180 parse, so quoting, embedded separators and embedded newlines never break a record, the field separator round-trips, and ragged rows keep their length. By default whitespace means the full Unicode White_Space set — non-breaking space (U+00A0), narrow NBSP (U+202F), ideographic space (U+3000) and friends, exactly the characters that survive a copy-paste from a spreadsheet or a web page — set whitespace = 'ascii' to leave those as content. trim picks the ends to strip ('both' (default), 'leading', 'trailing', 'none'); internal picks the interior treatment ('collapse' (default), 'remove', 'keep' for a pure trim); columns limits the pass to named, positional or ranged columns ('name,city', '1,3-5'); header marks row 1 as a header and normalize_header (default on) decides whether it is rewritten too. Only whitespace changes — no cell is retyped, reordered, dropped or invented. Runs entirely in the sandbox; nothing is uploaded.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "csv-whitespace-normalizer", |a: Args| {
            gizza_ai_csv_whitespace_normalizer_core::normalize(
                &a.input,
                &a.delimiter,
                &a.trim,
                &a.internal,
                &a.whitespace,
                &a.columns,
                a.header,
                a.normalize_header,
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
    /// reviewed. Authored 2026-08-17 for the initial csv-whitespace-normalizer release.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "input": { "type": "string", "description": "The CSV/delimited table to normalize, as text. Quoted fields with embedded separators or newlines (RFC 4180) are preserved. Rows may be ragged — a short row stays short. Max 5,000,000 bytes." },
                    "delimiter": { "type": "string", "default": ",", "description": "Field separator: 'auto' to sniff it from the first line, a single character, or a name ('comma' (default), 'tab', 'semicolon', 'pipe'). The output uses the same separator as the input." },
                    "trim": { "type": "string", "enum": ["both", "leading", "trailing", "none"], "default": "both", "description": "Which end(s) of a cell lose their whitespace: 'both' (default), 'leading' (start only), 'trailing' (end only), or 'none' to leave both edges alone and rewrite only the interior. An edge run that survives is copied verbatim." },
                    "internal": { "type": "string", "enum": ["collapse", "remove", "keep"], "default": "collapse", "description": "What happens to whitespace BETWEEN the first and last non-whitespace character of a cell: 'collapse' (default) turns every run into one plain space, so 'Ada   Lovelace' becomes 'Ada Lovelace'; 'remove' deletes it, so 'AB 12 CD' becomes 'AB12CD' (for SKUs, IBANs, part numbers); 'keep' preserves interior spacing byte-for-byte, making this a pure trim." },
                    "whitespace": { "type": "string", "enum": ["unicode", "ascii"], "default": "unicode", "description": "Which characters count as whitespace: 'unicode' (default) is the full Unicode White_Space set, so a non-breaking space (U+00A0), narrow NBSP (U+202F) or ideographic space (U+3000) pasted out of a spreadsheet is normalized too; 'ascii' restricts it to space, tab, newline, carriage return and form feed, leaving NBSP and friends as content. Zero-width characters (U+200B, U+FEFF) are not whitespace under either setting." },
                    "columns": { "type": "string", "default": "", "description": "Restrict the pass to specific columns: a comma-separated list of column names (needs header = true), 1-based positions, and inclusive ranges, e.g. 'name,city' or '1,3-5'. Default (empty) normalizes every column. A token that parses as a number or a range is read as a position, so a header literally named '3' must be selected by its position." },
                    "header": { "type": "boolean", "default": true, "description": "When true (default), the first row is a header: it supplies the names used by the columns option. Turn it off for a headerless table, where row 1 is data like any other." },
                    "normalize_header": { "type": "boolean", "default": true, "description": "When true (default), header cells get the same whitespace pass as the data, which is what makes ' first name ' load as 'first name'. Turn it off to copy the header row through byte-for-byte. Has no effect when header is false." }
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
