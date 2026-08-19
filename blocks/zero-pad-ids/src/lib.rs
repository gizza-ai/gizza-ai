//! gizza-ai/zero-pad-ids — chat skill block on the shared tool abstraction.
//! Pads (or strips) leading zeros on an ID/code column of a delimited table to a
//! fixed width, so codes sort lexicographically, join against a reference file,
//! and match a fixed-width spec again. The chat schema is single-sourced from
//! `descriptor()` (which also drives the CLI); `handle()` delegates to
//! `block_utils::run_skill`. Pure compute — nothing is uploaded.
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
    columns: String,
    #[serde(default)]
    width: i64,
    #[serde(default)]
    mode: String,
    #[serde(default)]
    overflow: String,
    #[serde(default)]
    non_numeric: String,
    #[serde(default = "default_true")]
    header: bool,
    #[serde(default)]
    quote_style: String,
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
                .describe("The table to fix, as text — CSV/TSV, or a plain one-ID-per-line list (which is just a one-column table). Quoted fields with embedded separators or newlines (RFC 4180) are preserved, and ragged rows stay ragged. Example: 'id,name\\n42,ada\\n7,linus'. Max 5,000,000 bytes."),
        )
        .param(
            Param::string("delimiter")
                .default("comma")
                .describe("Field separator: 'auto' to sniff it from the first line, a single character, or a name ('comma' (default), 'tab', 'semicolon', 'pipe'). The output uses the same separator as the input. A one-value-per-line list works with any setting."),
        )
        .param(
            Param::string("columns")
                .default("")
                .describe("Which columns to rewrite: a comma-separated list of column names (needs header = true) or 1-based positions, e.g. 'id,sku' or '1,3'. Default (empty) rewrites every column — name the ID column when the table also holds real numbers such as prices or quantities, which you do not want zero-padded."),
        )
        .param(
            Param::integer("width")
                .default(0)
                .min(0.0)
                .max(64.0)
                .describe("Target width in characters, e.g. 5 for a US ZIP code, 8 for an 8-digit SKU. 0 (the default) means auto: each selected column is padded up to its own widest eligible value, which is what you want when the codes were all one length before a loader ate the zeros. Ignored when mode is 'strip'. Max 64."),
        )
        .param(
            Param::enumv("mode", ["pad", "strip"])
                .default("pad")
                .describe("Direction: 'pad' (default) left-pads values with zeros up to width; 'strip' removes every leading zero instead ('00042' becomes '42', '000' becomes '0'), for when a fixed-width export needs to go back to plain numbers. width and overflow are unused in 'strip'."),
        )
        .param(
            Param::enumv("overflow", ["keep", "strip", "error"])
                .default("keep")
                .describe("What to do with a value already at or over width (pad mode only): 'keep' (default) leaves it exactly as it is; 'strip' drops its excess leading zeros so it lands on the width when possible ('0000012' at width 5 becomes '00012'); 'error' fails and names the row, column and value. Real digits are never truncated, so '123456' at width 5 survives intact under every setting."),
        )
        .param(
            Param::enumv("non_numeric", ["keep", "pad", "error"])
                .default("keep")
                .describe("What to do with a cell that is not a plain run of digits — 'SKU-9', 'N/A', '-42', '1.5': 'keep' (default) copies it through untouched; 'pad' pads it anyway, for alphanumeric codes ('AB12' at width 6 becomes '00AB12'); 'error' fails and names the row, column and value. Blank cells are always left blank under every setting — a missing ID is never invented into '00000'."),
        )
        .param(
            Param::boolean("header")
                .default(true)
                .describe("When true (default), the first row is a header: it is copied through untouched, so a column literally named '2024' is not padded, and it supplies the names used by the columns option. Turn it off for a bare list or a headerless export."),
        )
        .param(
            Param::enumv("quote_style", ["minimal", "always", "never"])
                .default("minimal")
                .describe("Output quoting: 'minimal' (default) quotes only fields that need it; 'always' quotes every field, which makes some spreadsheets and loaders read the padded codes as text instead of stripping the zeros again; 'never' emits bare fields, which is compact but can produce ambiguous CSV when values contain the separator."),
        )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/zero-pad-ids",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Pad or strip leading zeros on an ID/code column to a fixed width so codes sort and join correctly.",
    skill(
        description = "Fix an identifier column whose leading zeros were eaten (or added) somewhere upstream. A spreadsheet or loader typed the column as a number, so '00042' came back as '42' and the codes stopped sorting lexicographically, stopped joining against a reference file, and stopped matching a fixed-width spec. This left-pads the selected column(s) of a CSV/TSV table — or a plain one-ID-per-line list — with zeros up to a fixed width, and can also run the other way and strip leading zeros. Only the zeros change: cells outside the selected columns, blank cells, and (by default) cells that are not plain digits are copied through untouched, the header row is never rewritten, quoted fields keep their quoting, ragged rows keep their length, and the separator round-trips unchanged. Real digits are never truncated to make a value fit. delimiter accepts 'auto' (sniffed from the first line), a single char, or 'comma'/'tab'/'semicolon'/'pipe'. columns takes names (with header on) or 1-based positions; empty means every column, so name the ID column when the table also holds prices or quantities. width is the target length (0 = auto-fit each column to its widest value; max 64). mode is 'pad' or 'strip'. overflow decides what happens to a value already at or over the width: 'keep', 'strip' its excess leading zeros, or 'error'. non_numeric decides what happens to a cell that is not all digits: 'keep', 'pad' it anyway (for alphanumeric codes), or 'error'. header (default on) protects row 1. quote_style is 'minimal', 'always' (helps downstream readers treat the codes as text), or 'never'. Runs entirely in the sandbox; nothing is uploaded.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "zero-pad-ids", |a: Args| {
            gizza_ai_zero_pad_ids_core::zero_pad(
                &a.input,
                &a.delimiter,
                &a.columns,
                a.width,
                &a.mode,
                &a.overflow,
                &a.non_numeric,
                a.header,
                &a.quote_style,
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
    /// reviewed. Authored 2026-08-17 for the initial zero-pad-ids release.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "input": { "type": "string", "description": "The table to fix, as text — CSV/TSV, or a plain one-ID-per-line list (which is just a one-column table). Quoted fields with embedded separators or newlines (RFC 4180) are preserved, and ragged rows stay ragged. Example: 'id,name\\n42,ada\\n7,linus'. Max 5,000,000 bytes." },
                    "delimiter": { "type": "string", "default": "comma", "description": "Field separator: 'auto' to sniff it from the first line, a single character, or a name ('comma' (default), 'tab', 'semicolon', 'pipe'). The output uses the same separator as the input. A one-value-per-line list works with any setting." },
                    "columns": { "type": "string", "default": "", "description": "Which columns to rewrite: a comma-separated list of column names (needs header = true) or 1-based positions, e.g. 'id,sku' or '1,3'. Default (empty) rewrites every column — name the ID column when the table also holds real numbers such as prices or quantities, which you do not want zero-padded." },
                    "width": { "type": "integer", "default": 0, "minimum": 0, "maximum": 64, "description": "Target width in characters, e.g. 5 for a US ZIP code, 8 for an 8-digit SKU. 0 (the default) means auto: each selected column is padded up to its own widest eligible value, which is what you want when the codes were all one length before a loader ate the zeros. Ignored when mode is 'strip'. Max 64." },
                    "mode": { "type": "string", "enum": ["pad", "strip"], "default": "pad", "description": "Direction: 'pad' (default) left-pads values with zeros up to width; 'strip' removes every leading zero instead ('00042' becomes '42', '000' becomes '0'), for when a fixed-width export needs to go back to plain numbers. width and overflow are unused in 'strip'." },
                    "overflow": { "type": "string", "enum": ["keep", "strip", "error"], "default": "keep", "description": "What to do with a value already at or over width (pad mode only): 'keep' (default) leaves it exactly as it is; 'strip' drops its excess leading zeros so it lands on the width when possible ('0000012' at width 5 becomes '00012'); 'error' fails and names the row, column and value. Real digits are never truncated, so '123456' at width 5 survives intact under every setting." },
                    "non_numeric": { "type": "string", "enum": ["keep", "pad", "error"], "default": "keep", "description": "What to do with a cell that is not a plain run of digits — 'SKU-9', 'N/A', '-42', '1.5': 'keep' (default) copies it through untouched; 'pad' pads it anyway, for alphanumeric codes ('AB12' at width 6 becomes '00AB12'); 'error' fails and names the row, column and value. Blank cells are always left blank under every setting — a missing ID is never invented into '00000'." },
                    "header": { "type": "boolean", "default": true, "description": "When true (default), the first row is a header: it is copied through untouched, so a column literally named '2024' is not padded, and it supplies the names used by the columns option. Turn it off for a bare list or a headerless export." },
                    "quote_style": { "type": "string", "enum": ["minimal", "always", "never"], "default": "minimal", "description": "Output quoting: 'minimal' (default) quotes only fields that need it; 'always' quotes every field, which makes some spreadsheets and loaders read the padded codes as text instead of stripping the zeros again; 'never' emits bare fields, which is compact but can produce ambiguous CSV when values contain the separator." }
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
