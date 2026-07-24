//! gizza-ai/fastq-to-fasta — strips per-base quality lines from FASTQ reads and
//! emits clean FASTA, with optional length + mean-quality filtering, line
//! wrapping, header renaming, N-read discarding, and uppercasing.
//!
//! Chat-skill wrapper around `gizza-ai-fastq-to-fasta-core`. The chat schema is
//! derived from `descriptor()` (single source — shared shape across chat + CLI);
//! the handler delegates to `block_utils::run_skill`. No host calls — runs
//! entirely inside the WASM sandbox.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_fastq_to_fasta_core::{convert, Options, MAX_WRAP};
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    /// The FASTQ text (one or more 4-line reads).
    fastq: String,
    /// Drop reads shorter than this many bases (0 = no length filter).
    #[serde(default)]
    min_length: u32,
    /// Drop reads whose mean Phred quality is below this (0 = no quality filter).
    #[serde(default)]
    min_quality: f64,
    /// Phred ASCII offset, "33" (Sanger/Illumina 1.8+) or "64" (old Illumina).
    #[serde(default)]
    quality_offset: String,
    /// Wrap sequence lines at this many chars (0 = one line per sequence).
    #[serde(default)]
    wrap: u32,
    /// Replace each header with a sequential number.
    #[serde(default)]
    rename: bool,
    /// Discard reads that contain an ambiguous N base.
    #[serde(default)]
    strip_n: bool,
    /// Uppercase the sequence bases.
    #[serde(default)]
    uppercase: bool,
}

/// Parse the `quality_offset` enum string to the byte offset (blank → 33).
fn parse_offset(s: &str) -> Result<u8, String> {
    match s {
        "" | "33" => Ok(33),
        "64" => Ok(64),
        other => Err(format!(
            "invalid quality_offset {other:?}: expected \"33\" or \"64\""
        )),
    }
}

/// Single-source param descriptor → chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("fastq")
                .required()
                .describe("The FASTQ text to convert — one or more reads, each 4 lines: '@id [description]', the sequence, a '+' line, and the per-base quality string."),
        )
        .param(
            Param::integer("min_length")
                .default(0)
                .min(0.0)
                .describe("Drop reads shorter than this many bases. Default 0 (no length filter)."),
        )
        .param(
            Param::number("min_quality")
                .default(0.0)
                .min(0.0)
                .max(60.0)
                .describe("Drop reads whose MEAN Phred quality is below this, using quality_offset to decode. Default 0 (no quality filter)."),
        )
        .param(
            Param::enumv("quality_offset", ["33", "64"])
                .default("33")
                .describe("Phred ASCII offset for decoding quality: '33' (Sanger / Illumina 1.8+, the modern default) or '64' (old Illumina 1.3-1.5). Only affects the min_quality filter."),
        )
        .param(
            Param::integer("wrap")
                .default(0)
                .min(0.0)
                .max(MAX_WRAP as f64)
                .describe("Wrap each output sequence at this many characters per line (1-1000; e.g. 60 or 70). Default 0 = one line per sequence."),
        )
        .param(
            Param::boolean("rename")
                .default(false)
                .describe("When true, replace every header with a sequential number ('>1', '>2', ...). Default false (keep original headers)."),
        )
        .param(
            Param::boolean("strip_n")
                .default(false)
                .describe("When true, discard any read whose sequence contains an ambiguous 'N' base. Default false (keep N-containing reads)."),
        )
        .param(
            Param::boolean("uppercase")
                .default(false)
                .describe("When true, uppercase the sequence bases (acgt -> ACGT). Default false."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

/// Shared entry point: build the core `Options` from the parsed args and convert.
fn run(a: Args) -> Result<String, String> {
    let opts = Options {
        min_length: a.min_length as usize,
        min_quality: a.min_quality,
        quality_offset: parse_offset(&a.quality_offset)?,
        wrap: a.wrap as usize,
        rename: a.rename,
        strip_n: a.strip_n,
        uppercase: a.uppercase,
    };
    convert(&a.fastq, &opts)
}

#[cfg(target_arch = "wasm32")]
struct FastqToFasta;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/fastq-to-fasta",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Convert FASTQ reads to clean FASTA, dropping the per-base quality lines.",
    skill(
        description = "Convert FASTQ sequencing reads to FASTA by stripping the per-base quality lines and rewriting '@' headers as '>'. Each FASTQ read is 4 lines (header, sequence, '+', quality); the output keeps only the header and sequence. Optional filters: min_length drops reads shorter than N bases; min_quality drops reads whose mean Phred quality (decoded with quality_offset 33 or 64) is below a threshold; strip_n discards reads containing ambiguous N bases. rename replaces headers with sequential numbers, uppercase normalizes the bases, and wrap splits each sequence into fixed-width lines (e.g. 60 or 70). Runs entirely locally.",
        parameters = schema_json()
    ),
)]
impl FastqToFasta {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "fastq-to-fasta", |a: Args| run(a).map_err(SkillError::InvalidArgs)) {
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
                    "fastq": { "type": "string", "description": "The FASTQ text to convert — one or more reads, each 4 lines: '@id [description]', the sequence, a '+' line, and the per-base quality string." },
                    "min_length": { "type": "integer", "minimum": 0, "default": 0, "description": "Drop reads shorter than this many bases. Default 0 (no length filter)." },
                    "min_quality": { "type": "number", "minimum": 0, "maximum": 60, "default": 0.0, "description": "Drop reads whose MEAN Phred quality is below this, using quality_offset to decode. Default 0 (no quality filter)." },
                    "quality_offset": { "type": "string", "enum": ["33", "64"], "default": "33", "description": "Phred ASCII offset for decoding quality: '33' (Sanger / Illumina 1.8+, the modern default) or '64' (old Illumina 1.3-1.5). Only affects the min_quality filter." },
                    "wrap": { "type": "integer", "minimum": 0, "maximum": 1000, "default": 0, "description": "Wrap each output sequence at this many characters per line (1-1000; e.g. 60 or 70). Default 0 = one line per sequence." },
                    "rename": { "type": "boolean", "default": false, "description": "When true, replace every header with a sequential number ('>1', '>2', ...). Default false (keep original headers)." },
                    "strip_n": { "type": "boolean", "default": false, "description": "When true, discard any read whose sequence contains an ambiguous 'N' base. Default false (keep N-containing reads)." },
                    "uppercase": { "type": "boolean", "default": false, "description": "When true, uppercase the sequence bases (acgt -> ACGT). Default false." }
                },
                "required": ["fastq"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
