//! gizza-ai/phylip-to-fasta — converts PHYLIP sequential or interleaved
//! multiple-sequence-alignment text into standard FASTA.
//!
//! Chat-skill wrapper around `gizza-ai-phylip-to-fasta-core`. The chat schema is
//! derived from `descriptor()` (single source — shared shape across chat + CLI);
//! the handler delegates to `block_utils::run_skill`. No host calls — runs
//! entirely inside the WASM sandbox.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_phylip_to_fasta_core::{convert, Layout, NameStyle, Options, MAX_WRAP};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    /// The PHYLIP alignment text, starting with the `<taxa> <sites>` header.
    phylip: String,
    /// Body layout: "auto" (default), "sequential" or "interleaved".
    #[serde(default)]
    layout: String,
    /// Taxon-name style: "auto" (default), "strict" or "relaxed".
    #[serde(default)]
    name_style: String,
    /// FASTA line width (0 = one line per sequence).
    #[serde(default = "default_wrap")]
    wrap: u32,
    /// Uppercase the sequence residues.
    #[serde(default)]
    uppercase: bool,
    /// Strip the gap characters `-` and `.`.
    #[serde(default)]
    remove_gaps: bool,
    /// Accept a file that disagrees with its own header.
    #[serde(default)]
    tolerant: bool,
}

/// Keep the serde default in step with the descriptor's `.default(60)`.
fn default_wrap() -> u32 {
    60
}

/// Parse the `layout` enum string (blank → auto).
fn parse_layout(s: &str) -> Result<Layout, String> {
    match s {
        "" | "auto" => Ok(Layout::Auto),
        "sequential" => Ok(Layout::Sequential),
        "interleaved" => Ok(Layout::Interleaved),
        other => Err(format!(
            "invalid layout {other:?}: expected \"auto\", \"sequential\" or \"interleaved\""
        )),
    }
}

/// Parse the `name_style` enum string (blank → auto).
fn parse_name_style(s: &str) -> Result<NameStyle, String> {
    match s {
        "" | "auto" => Ok(NameStyle::Auto),
        "strict" => Ok(NameStyle::Strict),
        "relaxed" => Ok(NameStyle::Relaxed),
        other => Err(format!(
            "invalid name_style {other:?}: expected \"auto\", \"strict\" or \"relaxed\""
        )),
    }
}

/// Single-source param descriptor → chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("phylip")
                .required()
                .describe("The PHYLIP alignment text. The first line is the count header '<taxa> <sites>' (e.g. '3 12'); the lines after it hold each taxon's name and sequence, either sequential (one taxon at a time) or interleaved (blocks of one chunk per taxon)."),
        )
        .param(
            Param::enumv("layout", ["auto", "sequential", "interleaved"])
                .default("auto")
                .describe("Body layout. 'auto' (default) detects it from the block structure and checks the result against the declared site count; 'sequential' = each taxon's whole sequence before the next; 'interleaved' = repeated blocks of one chunk per taxon."),
        )
        .param(
            Param::enumv("name_style", ["auto", "strict", "relaxed"])
                .default("auto")
                .describe("How the taxon name is separated from the sequence. 'auto' (default) tries relaxed then strict and keeps the parse that matches the site count; 'strict' = the name is exactly columns 1-10; 'relaxed' = the name is the first whitespace-delimited token, so it may exceed 10 characters (RAxML/PhyML style)."),
        )
        .param(
            Param::integer("wrap")
                .default(60)
                .min(0.0)
                .max(MAX_WRAP as f64)
                .describe("Wrap each FASTA sequence at this many characters per line (1-1000). Default 60, the conventional FASTA width; 0 = one long line per sequence."),
        )
        .param(
            Param::boolean("uppercase")
                .default(false)
                .describe("When true, uppercase the sequence residues (acgt -> ACGT). Default false (keep the original case)."),
        )
        .param(
            Param::boolean("remove_gaps")
                .default(false)
                .describe("When true, strip the alignment gap characters '-' and '.', turning the aligned FASTA into unaligned sequences. Default false (gaps are preserved exactly)."),
        )
        .param(
            Param::boolean("tolerant")
                .default(false)
                .describe("When true, convert anyway if the file disagrees with its own header (wrong taxon count, wrong sequence length, or unexpected residue characters). Default false = report the mismatch instead."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

/// Shared entry point: build the core `Options` from the parsed args and convert.
fn run(a: Args) -> Result<String, String> {
    let opts = Options {
        layout: parse_layout(&a.layout)?,
        name_style: parse_name_style(&a.name_style)?,
        wrap: a.wrap as usize,
        uppercase: a.uppercase,
        remove_gaps: a.remove_gaps,
        tolerant: a.tolerant,
    };
    convert(&a.phylip, &opts)
}

#[cfg(target_arch = "wasm32")]
struct PhylipToFasta;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/phylip-to-fasta",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Convert PHYLIP sequential or interleaved alignments into standard FASTA.",
    skill(
        description = "Convert a PHYLIP multiple-sequence alignment into standard FASTA. PHYLIP files start with a '<taxa> <sites>' count header, then either the sequential layout (each taxon's whole sequence before the next) or the interleaved layout (repeated blocks holding one chunk per taxon); taxon names are either strict (columns 1-10) or relaxed (the first whitespace-delimited token). Both layouts and both name styles are auto-detected by checking each candidate parse against the declared site count, and can also be forced with the layout and name_style parameters. Gaps are preserved by default; remove_gaps strips '-' and '.' to produce unaligned sequences, uppercase normalizes residues, wrap sets the FASTA line width (default 60, 0 = one line), and tolerant converts anyway when the file disagrees with its own header. Runs entirely locally.",
        parameters = schema_json()
    ),
)]
impl PhylipToFasta {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "phylip-to-fasta", |a: Args| {
            run(a).map_err(SkillError::InvalidArgs)
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
    /// schema, so any future change to the LLM-facing API is intentional.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "phylip": { "type": "string", "description": "The PHYLIP alignment text. The first line is the count header '<taxa> <sites>' (e.g. '3 12'); the lines after it hold each taxon's name and sequence, either sequential (one taxon at a time) or interleaved (blocks of one chunk per taxon)." },
                    "layout": { "type": "string", "enum": ["auto", "sequential", "interleaved"], "default": "auto", "description": "Body layout. 'auto' (default) detects it from the block structure and checks the result against the declared site count; 'sequential' = each taxon's whole sequence before the next; 'interleaved' = repeated blocks of one chunk per taxon." },
                    "name_style": { "type": "string", "enum": ["auto", "strict", "relaxed"], "default": "auto", "description": "How the taxon name is separated from the sequence. 'auto' (default) tries relaxed then strict and keeps the parse that matches the site count; 'strict' = the name is exactly columns 1-10; 'relaxed' = the name is the first whitespace-delimited token, so it may exceed 10 characters (RAxML/PhyML style)." },
                    "wrap": { "type": "integer", "minimum": 0, "maximum": 1000, "default": 60, "description": "Wrap each FASTA sequence at this many characters per line (1-1000). Default 60, the conventional FASTA width; 0 = one long line per sequence." },
                    "uppercase": { "type": "boolean", "default": false, "description": "When true, uppercase the sequence residues (acgt -> ACGT). Default false (keep the original case)." },
                    "remove_gaps": { "type": "boolean", "default": false, "description": "When true, strip the alignment gap characters '-' and '.', turning the aligned FASTA into unaligned sequences. Default false (gaps are preserved exactly)." },
                    "tolerant": { "type": "boolean", "default": false, "description": "When true, convert anyway if the file disagrees with its own header (wrong taxon count, wrong sequence length, or unexpected residue characters). Default false = report the mismatch instead." }
                },
                "required": ["phylip"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }

    #[test]
    fn run_converts_an_interleaved_alignment() {
        let a = Args {
            phylip: "2 8\nAlpha     ACGT\nBeta      TTTT\n\nACGT\nTTTT\n".to_string(),
            layout: String::new(),
            name_style: String::new(),
            wrap: 0,
            uppercase: false,
            remove_gaps: false,
            tolerant: false,
        };
        assert_eq!(run(a).unwrap(), ">Alpha\nACGTACGT\n>Beta\nTTTTTTTT\n");
    }

    #[test]
    fn run_rejects_an_unknown_layout() {
        let a = Args {
            phylip: "1 4\nAlpha     ACGT\n".to_string(),
            layout: "columnar".to_string(),
            name_style: String::new(),
            wrap: 0,
            uppercase: false,
            remove_gaps: false,
            tolerant: false,
        };
        let err = run(a).unwrap_err();
        assert!(err.contains("invalid layout"), "got: {err}");
    }

    #[test]
    fn run_rejects_an_unknown_name_style() {
        let a = Args {
            phylip: "1 4\nAlpha     ACGT\n".to_string(),
            layout: String::new(),
            name_style: "fixed".to_string(),
            wrap: 0,
            uppercase: false,
            remove_gaps: false,
            tolerant: false,
        };
        let err = run(a).unwrap_err();
        assert!(err.contains("invalid name_style"), "got: {err}");
    }
}
