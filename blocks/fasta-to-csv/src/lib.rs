//! gizza-ai/fasta-to-csv — parse FASTA text into a delimited table (CSV/TSV) of
//! id, description, sequence and length, with optional GC-content and per-base
//! count columns, uppercasing and sequence deduplication.
//!
//! Chat-skill wrapper around `gizza-ai-fasta-to-csv-core`. The chat schema is
//! derived from `descriptor()` (single source — shared shape across chat + CLI);
//! the handler delegates to `block_utils::run_skill`. No host calls — runs
//! entirely inside the WASM sandbox.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_fasta_to_csv_core::{convert, Delimiter, HeaderMode, Options};
use serde::Deserialize;
use wafer_sdk::*;

/// `serde` default for the two booleans that default to ON.
fn yes() -> bool {
    true
}

#[derive(Deserialize)]
struct Args {
    /// The FASTA text (one or more `>header` + sequence records).
    fasta: String,
    /// Output field separator: comma | tab | semicolon | pipe.
    #[serde(default)]
    delimiter: String,
    /// How the header becomes columns: split | id_only | full_header.
    #[serde(default)]
    header_mode: String,
    /// Emit a header row naming the columns.
    #[serde(default = "yes")]
    header_row: bool,
    /// Emit the `sequence` column.
    #[serde(default = "yes")]
    include_sequence: bool,
    /// Emit the `length` column.
    #[serde(default = "yes")]
    include_length: bool,
    /// Emit the `gc_percent` column.
    #[serde(default)]
    include_gc: bool,
    /// Emit the five per-base count columns.
    #[serde(default)]
    include_base_counts: bool,
    /// Uppercase the emitted sequence.
    #[serde(default)]
    uppercase: bool,
    /// Drop records whose sequence already appeared.
    #[serde(default)]
    dedupe: bool,
}

/// Single-source param descriptor → chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("fasta")
                .required()
                .describe("The FASTA text to convert — one or more records, each a '>id description' header line followed by one or more sequence lines (wrapped lines are joined). Example: a line '>seq1 first sequence' followed by a line 'ACGTACGTNN'."),
        )
        .param(
            Param::enumv("delimiter", ["comma", "tab", "semicolon", "pipe"])
                .default("comma")
                .describe("Field separator for the output table: 'comma' (CSV, the default), 'tab' (TSV), 'semicolon' (spreadsheets in comma-decimal locales) or 'pipe'."),
        )
        .param(
            Param::enumv("header_mode", ["split", "id_only", "full_header"])
                .default("split")
                .describe("How the '>' header becomes columns: 'split' (default) puts the text before the first space in 'id' and the rest in a 'description' column; 'id_only' keeps just the id and drops the description column; 'full_header' puts the entire header line into 'id' with no description column."),
        )
        .param(
            Param::boolean("header_row")
                .default(true)
                .describe("When true (default), the first output row names the columns (id, description, sequence, length, ...). Set false for a bare data table."),
        )
        .param(
            Param::boolean("include_sequence")
                .default(true)
                .describe("When true (default), include the joined sequence as a column. Set false for a names/metrics-only table."),
        )
        .param(
            Param::boolean("include_length")
                .default(true)
                .describe("When true (default), include a 'length' column with the number of sequence characters (gaps and ambiguity codes such as N are counted)."),
        )
        .param(
            Param::boolean("include_gc")
                .default(false)
                .describe("When true, add a 'gc_percent' column: (G+C)/(A+C+G+T) x 100 rounded to 2 decimals, case-insensitive, ignoring N and gaps. Default false."),
        )
        .param(
            Param::boolean("include_base_counts")
                .default(false)
                .describe("When true, add five columns — a_count, c_count, g_count, t_count and other_count (N, gaps, amino acids, anything not A/C/G/T). Case-insensitive. Default false."),
        )
        .param(
            Param::boolean("uppercase")
                .default(false)
                .describe("When true, uppercase the emitted sequence column (acgt -> ACGT). Does not change lengths, GC or base counts, which are already case-insensitive. Default false."),
        )
        .param(
            Param::boolean("dedupe")
                .default(false)
                .describe("When true, drop any record whose sequence (compared case-insensitively) already appeared, keeping the first occurrence. Default false."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

/// Shared entry point: build the core `Options` from the parsed args and convert.
fn run(a: Args) -> Result<String, String> {
    let opts = Options {
        delimiter: Delimiter::parse(&a.delimiter)?,
        header_mode: HeaderMode::parse(&a.header_mode)?,
        header_row: a.header_row,
        include_sequence: a.include_sequence,
        include_length: a.include_length,
        include_gc: a.include_gc,
        include_base_counts: a.include_base_counts,
        uppercase: a.uppercase,
        dedupe: a.dedupe,
    };
    convert(&a.fasta, &opts)
}

#[cfg(target_arch = "wasm32")]
struct FastaToCsv;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/fasta-to-csv",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Parse FASTA records into a CSV or TSV table of id, description, sequence and length.",
    skill(
        description = "Convert FASTA sequence text into a delimited table (CSV by default, or TSV/semicolon/pipe) with one row per record. The default columns are id, description, sequence and length; wrapped sequence lines are joined into one field and every field is RFC-4180 quoted when it contains the delimiter, a quote or a line break. header_mode chooses how the '>' header maps to columns (split into id + description, id only, or the whole header line). include_sequence and include_length toggle those columns; include_gc adds GC percentage and include_base_counts adds A/C/G/T/other counts. uppercase normalizes the sequence and dedupe drops repeated sequences. Handles up to 50000 records and runs entirely locally.",
        parameters = schema_json()
    ),
)]
impl FastaToCsv {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "fasta-to-csv", |a: Args| run(a).map_err(SkillError::InvalidArgs)) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Drift guard: the descriptor-derived chat schema must match this authored
    /// schema, so any future change to the LLM-facing API is intentional.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "fasta": { "type": "string", "description": "The FASTA text to convert — one or more records, each a '>id description' header line followed by one or more sequence lines (wrapped lines are joined). Example: a line '>seq1 first sequence' followed by a line 'ACGTACGTNN'." },
                    "delimiter": { "type": "string", "enum": ["comma", "tab", "semicolon", "pipe"], "default": "comma", "description": "Field separator for the output table: 'comma' (CSV, the default), 'tab' (TSV), 'semicolon' (spreadsheets in comma-decimal locales) or 'pipe'." },
                    "header_mode": { "type": "string", "enum": ["split", "id_only", "full_header"], "default": "split", "description": "How the '>' header becomes columns: 'split' (default) puts the text before the first space in 'id' and the rest in a 'description' column; 'id_only' keeps just the id and drops the description column; 'full_header' puts the entire header line into 'id' with no description column." },
                    "header_row": { "type": "boolean", "default": true, "description": "When true (default), the first output row names the columns (id, description, sequence, length, ...). Set false for a bare data table." },
                    "include_sequence": { "type": "boolean", "default": true, "description": "When true (default), include the joined sequence as a column. Set false for a names/metrics-only table." },
                    "include_length": { "type": "boolean", "default": true, "description": "When true (default), include a 'length' column with the number of sequence characters (gaps and ambiguity codes such as N are counted)." },
                    "include_gc": { "type": "boolean", "default": false, "description": "When true, add a 'gc_percent' column: (G+C)/(A+C+G+T) x 100 rounded to 2 decimals, case-insensitive, ignoring N and gaps. Default false." },
                    "include_base_counts": { "type": "boolean", "default": false, "description": "When true, add five columns — a_count, c_count, g_count, t_count and other_count (N, gaps, amino acids, anything not A/C/G/T). Case-insensitive. Default false." },
                    "uppercase": { "type": "boolean", "default": false, "description": "When true, uppercase the emitted sequence column (acgt -> ACGT). Does not change lengths, GC or base counts, which are already case-insensitive. Default false." },
                    "dedupe": { "type": "boolean", "default": false, "description": "When true, drop any record whose sequence (compared case-insensitively) already appeared, keeping the first occurrence. Default false." }
                },
                "required": ["fasta"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
