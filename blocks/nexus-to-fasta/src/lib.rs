//! gizza-ai/nexus-to-fasta — extracts the sequence matrix from a NEXUS file's
//! DATA/CHARACTERS block and writes it out as FASTA.
//!
//! Chat-skill wrapper around `gizza-ai-nexus-to-fasta-core`. The chat schema is
//! derived from `descriptor()` (single source — shared shape across chat + CLI);
//! the handler delegates to `block_utils::run_skill`. No host calls — runs
//! entirely inside the WASM sandbox.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_nexus_to_fasta_core::{convert, Case, Layout, Options, MAX_WRAP};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    /// The NEXUS document, including its `begin data;` … `end;` block.
    nexus: String,
    /// Matrix layout: "auto" (default), "sequential" or "interleaved".
    #[serde(default)]
    layout: String,
    /// FASTA line width (0 = one line per sequence).
    #[serde(default = "default_wrap")]
    wrap: u32,
    /// Residue case: "keep" (default), "upper" or "lower".
    #[serde(default)]
    case: String,
    /// Strip the alignment gap characters.
    #[serde(default)]
    remove_gaps: bool,
    /// Expand the declared `matchchar` symbol against the first taxon.
    #[serde(default = "default_true")]
    expand_matchchar: bool,
    /// Turn `_` in unquoted taxon labels into spaces.
    #[serde(default)]
    underscores_to_spaces: bool,
    /// Accept a file that disagrees with its own `dimensions` command.
    #[serde(default)]
    tolerant: bool,
}

/// Keep the serde default in step with the descriptor's `.default(60)`.
fn default_wrap() -> u32 {
    60
}

/// Keep the serde default in step with `expand_matchchar`'s `.default(true)`.
fn default_true() -> bool {
    true
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

/// Parse the `case` enum string (blank → keep).
fn parse_case(s: &str) -> Result<Case, String> {
    match s {
        "" | "keep" => Ok(Case::Keep),
        "upper" => Ok(Case::Upper),
        "lower" => Ok(Case::Lower),
        other => Err(format!(
            "invalid case {other:?}: expected \"keep\", \"upper\" or \"lower\""
        )),
    }
}

/// Single-source param descriptor → chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("nexus")
                .required()
                .describe("The NEXUS document text. It starts with '#NEXUS' and must contain a 'begin data;' (or 'begin characters;') block holding the 'dimensions', 'format' and 'matrix' commands; bracketed [comments] and any other blocks (TAXA, TREES, ASSUMPTIONS) are ignored."),
        )
        .param(
            Param::enumv("layout", ["auto", "sequential", "interleaved"])
                .default("auto")
                .describe("Matrix layout. 'auto' (default) honours the format command's 'interleave' flag and otherwise keeps whichever parse matches the declared nchar; 'sequential' = each taxon's whole sequence before the next; 'interleaved' = repeated blocks of one row per taxon."),
        )
        .param(
            Param::integer("wrap")
                .default(60)
                .min(0.0)
                .max(MAX_WRAP as f64)
                .describe("Wrap each FASTA sequence at this many characters per line (1-1000). Default 60, the conventional FASTA width; 0 = one long line per sequence."),
        )
        .param(
            Param::enumv("case", ["keep", "upper", "lower"])
                .default("keep")
                .describe("Residue case in the output. 'keep' (default) copies the matrix verbatim; 'upper' normalizes to ACGT; 'lower' normalizes to acgt."),
        )
        .param(
            Param::boolean("remove_gaps")
                .default(false)
                .describe("When true, strip the alignment gap characters (the block's declared gap= symbol plus '-' and '.'), turning the aligned FASTA into unaligned sequences. Default false (gaps are preserved exactly)."),
        )
        .param(
            Param::boolean("expand_matchchar")
                .default(true)
                .describe("When true (default), replace each occurrence of the block's declared matchchar= symbol (usually '.') with the residue the first taxon has at that site, which is what the symbol means. Set false to copy the symbol through unchanged."),
        )
        .param(
            Param::boolean("underscores_to_spaces")
                .default(false)
                .describe("When true, turn '_' in an UNQUOTED taxon label into a space, the NEXUS convention ('Homo_sapiens' -> 'Homo sapiens'). Single-quoted labels are always taken literally. Default false (underscores are kept, which is safer for FASTA headers)."),
        )
        .param(
            Param::boolean("tolerant")
                .default(false)
                .describe("When true, convert anyway if the matrix disagrees with the dimensions command (wrong ntax, or a taxon whose sequence is not nchar sites long). Default false = report the mismatch instead."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

/// Shared entry point: build the core `Options` from the parsed args and convert.
fn run(a: Args) -> Result<String, String> {
    let opts = Options {
        layout: parse_layout(&a.layout)?,
        wrap: a.wrap as usize,
        case: parse_case(&a.case)?,
        remove_gaps: a.remove_gaps,
        expand_matchchar: a.expand_matchchar,
        underscores_to_spaces: a.underscores_to_spaces,
        tolerant: a.tolerant,
    };
    convert(&a.nexus, &opts)
}

#[cfg(target_arch = "wasm32")]
struct NexusToFasta;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/nexus-to-fasta",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Extract a NEXUS DATA/CHARACTERS matrix and write it out as FASTA.",
    skill(
        description = "Convert a NEXUS (PAUP/MrBayes/MEGA) alignment into standard FASTA. The tool strips bracketed [comments], finds the 'begin data;' or 'begin characters;' block, reads its dimensions (ntax/nchar) and format (gap, matchchar, interleave, labels) commands, and extracts the matrix — sequential or interleaved, auto-detected by checking each candidate parse against the declared nchar. Single-quoted taxon labels with spaces are supported, as are matchchar shorthand ('.' = same as the first taxon, expanded by default), non-default gap symbols, bracketed state sets like (01) that count as one site, and matrices with 'labels=no' whose names come from the TAXA block. Gaps are preserved by default; remove_gaps strips them for unaligned output, case normalizes residues, wrap sets the FASTA line width (default 60, 0 = one line), underscores_to_spaces applies the NEXUS underscore convention, and tolerant converts anyway when the matrix disagrees with its dimensions. Runs entirely locally.",
        parameters = schema_json()
    ),
)]
impl NexusToFasta {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "nexus-to-fasta", |a: Args| {
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

    fn args(nexus: &str) -> Args {
        Args {
            nexus: nexus.to_string(),
            layout: String::new(),
            wrap: 0,
            case: String::new(),
            remove_gaps: false,
            expand_matchchar: true,
            underscores_to_spaces: false,
            tolerant: false,
        }
    }

    /// Drift guard: the descriptor-derived chat schema must match this authored
    /// schema, so any future change to the LLM-facing API is intentional.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "nexus": { "type": "string", "description": "The NEXUS document text. It starts with '#NEXUS' and must contain a 'begin data;' (or 'begin characters;') block holding the 'dimensions', 'format' and 'matrix' commands; bracketed [comments] and any other blocks (TAXA, TREES, ASSUMPTIONS) are ignored." },
                    "layout": { "type": "string", "enum": ["auto", "sequential", "interleaved"], "default": "auto", "description": "Matrix layout. 'auto' (default) honours the format command's 'interleave' flag and otherwise keeps whichever parse matches the declared nchar; 'sequential' = each taxon's whole sequence before the next; 'interleaved' = repeated blocks of one row per taxon." },
                    "wrap": { "type": "integer", "minimum": 0, "maximum": 1000, "default": 60, "description": "Wrap each FASTA sequence at this many characters per line (1-1000). Default 60, the conventional FASTA width; 0 = one long line per sequence." },
                    "case": { "type": "string", "enum": ["keep", "upper", "lower"], "default": "keep", "description": "Residue case in the output. 'keep' (default) copies the matrix verbatim; 'upper' normalizes to ACGT; 'lower' normalizes to acgt." },
                    "remove_gaps": { "type": "boolean", "default": false, "description": "When true, strip the alignment gap characters (the block's declared gap= symbol plus '-' and '.'), turning the aligned FASTA into unaligned sequences. Default false (gaps are preserved exactly)." },
                    "expand_matchchar": { "type": "boolean", "default": true, "description": "When true (default), replace each occurrence of the block's declared matchchar= symbol (usually '.') with the residue the first taxon has at that site, which is what the symbol means. Set false to copy the symbol through unchanged." },
                    "underscores_to_spaces": { "type": "boolean", "default": false, "description": "When true, turn '_' in an UNQUOTED taxon label into a space, the NEXUS convention ('Homo_sapiens' -> 'Homo sapiens'). Single-quoted labels are always taken literally. Default false (underscores are kept, which is safer for FASTA headers)." },
                    "tolerant": { "type": "boolean", "default": false, "description": "When true, convert anyway if the matrix disagrees with the dimensions command (wrong ntax, or a taxon whose sequence is not nchar sites long). Default false = report the mismatch instead." }
                },
                "required": ["nexus"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }

    #[test]
    fn run_converts_a_data_block() {
        let a = args("#NEXUS\nbegin data;\n dimensions ntax=2 nchar=8;\n format datatype=dna gap=-;\n matrix\n  Alpha ACGTACGT\n  Beta  ACGTTCGT\n ;\nend;\n");
        assert_eq!(run(a).unwrap(), ">Alpha\nACGTACGT\n>Beta\nACGTTCGT\n");
    }

    #[test]
    fn run_rejects_an_unknown_layout() {
        let mut a = args("#NEXUS\nbegin data;\n matrix Alpha ACGT ;\nend;\n");
        a.layout = "matrix".to_string();
        assert!(run(a).unwrap_err().contains("invalid layout"));
    }

    #[test]
    fn run_rejects_an_unknown_case() {
        let mut a = args("#NEXUS\nbegin data;\n matrix Alpha ACGT ;\nend;\n");
        a.case = "title".to_string();
        assert!(run(a).unwrap_err().contains("invalid case"));
    }

    #[test]
    fn run_reports_a_missing_data_block() {
        let a = args("#NEXUS\nbegin trees;\n tree one = (a,b);\nend;\n");
        assert!(run(a).unwrap_err().contains("no 'begin data;'"));
    }
}
